//! Speech assist (roadmap item 27): phone models the person downloads on
//! purpose, kept in app data, and the worker thread that runs them. The
//! project is never touched here except through `session` functions, which
//! the session thread runs; recognition runs on its own thread so a slow
//! model never holds up the project.
//!
//! Models are an optional download, never bundled. Nothing leaves the
//! machine: the audio is read from the project's own blob store, and the
//! only network traffic is the model download the person starts.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use serde::Serialize;
use sha2::{Digest, Sha256};
use wugs_asr::{Recognition, Recognizer, Vocab};
use wugs_model::ulid;

/// One file of a model, as published.
#[derive(Clone, Debug)]
pub struct FileSpec {
    pub name: &'static str,
    pub url: String,
    /// The published SHA-256, when the publisher gives one. Without it the
    /// hash of the first download is recorded and shown.
    pub sha256: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub about: &'static str,
    pub license: &'static str,
    pub attribution: &'static str,
    pub homepage: &'static str,
    /// Rough download size, for the person deciding to fetch it.
    pub approx_mb: u32,
    pub files: Vec<FileSpec>,
    /// The ONNX file and the Hugging Face `vocab.json` among `files`.
    pub onnx: &'static str,
    pub vocab: &'static str,
    /// Whether the model wants each clip normalized (wav2vec2 does).
    pub normalize: bool,
}

/// The models this version knows how to run. Apache-2.0 weights only, so
/// running them inside an MPL app raises no question.
pub fn catalog() -> Vec<ModelSpec> {
    let hf = |repo: &str, file: &str| format!("https://huggingface.co/{repo}/resolve/main/{file}");
    vec![ModelSpec {
        id: "wav2vec2-espeak",
        name: "wav2vec 2.0 phones (espeak IPA)",
        about: "Meta's wav2vec2-lv-60-espeak-cv-ft phone recognizer, exported to ONNX. Writes the phones it hears in any language as IPA. Strongest on sounds common in its Common Voice training languages; expect to correct rarer segments.",
        license: "Apache-2.0",
        attribution: "Model © Meta Platforms (Xu, Baevski & Auli 2022); ONNX export by sadda-speech",
        homepage: "https://huggingface.co/sadda-speech/wav2vec2-espeak-ctc",
        approx_mb: 640,
        files: vec![
            FileSpec { name: "model.onnx", url: hf("sadda-speech/wav2vec2-espeak-ctc", "model.onnx"), sha256: None },
            FileSpec { name: "vocab.json", url: hf("sadda-speech/wav2vec2-espeak-ctc", "vocab.json"), sha256: None },
        ],
        onnx: "model.onnx",
        vocab: "vocab.json",
        normalize: true,
    }]
}

/// A file larger than this is refused mid-download.
const MAX_FILE_BYTES: u64 = 4 << 30;

/// ureq has no per-read stall timeout, so a stalled body ends only at this
/// ceiling; Cancel frees the slot at once in the meantime (see `Downloads`).
fn agent(loopback: bool) -> ureq::Agent {
    use std::time::Duration;
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .timeout_recv_body(Some(Duration::from_secs(3 * 60 * 60)))
        // Every hop must stay on HTTPS, redirects included.
        .https_only(!loopback)
        .build()
        .into()
}

fn model_dir(app_data: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
        return Err(format!("{id:?} is not a model id"));
    }
    Ok(app_data.join("models").join(id))
}

/// What a finished download recorded: each file's size and SHA-256.
#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub files: Vec<InstalledFile>,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFile {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
    /// Whether the hash matched one the publisher gave, or was recorded at download.
    pub published: bool,
}

/// The installed record, if the model finished downloading and its files are there.
pub fn installed(app_data: &Path, spec: &ModelSpec) -> Option<Installed> {
    let dir = model_dir(app_data, spec.id).ok()?;
    let rec: Installed = serde_json::from_slice(&fs::read(dir.join("installed.json")).ok()?).ok()?;
    let all_there = spec.files.iter().all(|f| rec.files.iter().any(|r| r.name == f.name) && dir.join(f.name).is_file());
    all_there.then_some(rec)
}

/// Only HTTPS, except to this machine (the tests' local server).
fn allowed_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:")
}

/// Download every file of `spec` into `<app data>/models/<id>/`. Each file
/// streams into a temporary name and is renamed only once complete and
/// checked; `installed.json` is written last, so a model is installed only
/// when all of it is. `progress(done, total)` counts bytes over all files
/// (total is 0 while unknown).
pub fn download(app_data: &Path, spec: &ModelSpec, cancel: &AtomicBool, progress: &dyn Fn(u64, u64)) -> Result<Installed, String> {
    let dir = model_dir(app_data, spec.id)?;
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    sweep_parts(&dir);
    let mut done_bytes = 0u64;
    let mut total = 0u64;
    let mut files = Vec::new();
    for f in &spec.files {
        if !allowed_url(&f.url) {
            return Err(format!("refusing to download {} over an insecure connection", f.url));
        }
        let loopback = !f.url.starts_with("https://");
        let mut resp = agent(loopback).get(&f.url).call().map_err(|e| format!("download {}: {e}", f.name))?;
        let len = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
        if let Some(len) = len {
            if len > MAX_FILE_BYTES {
                return Err(format!("{} is larger than {} GB", f.name, MAX_FILE_BYTES >> 30));
            }
            total += len;
        }
        let part = dir.join(format!(".{}.{}.part", f.name, ulid()));
        let result = (|| -> Result<(u64, String), String> {
            let mut out = fs::File::create(&part).map_err(|e| format!("write {}: {e}", part.display()))?;
            let mut reader = resp.body_mut().as_reader();
            let mut hasher = Sha256::new();
            let mut buf = vec![0u8; 1 << 16];
            let mut bytes = 0u64;
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err("cancelled".into());
                }
                let n = reader.read(&mut buf).map_err(|e| format!("download {}: {e}", f.name))?;
                if n == 0 {
                    break;
                }
                bytes += n as u64;
                if bytes > MAX_FILE_BYTES {
                    return Err(format!("{} is larger than {} GB", f.name, MAX_FILE_BYTES >> 30));
                }
                hasher.update(&buf[..n]);
                out.write_all(&buf[..n]).map_err(|e| format!("write {}: {e}", part.display()))?;
                progress(done_bytes + bytes, total.max(done_bytes + bytes));
            }
            if len.is_some_and(|l| l != bytes) {
                return Err(format!("{} arrived incomplete ({bytes} of {} bytes)", f.name, len.unwrap_or(0)));
            }
            out.sync_all().map_err(|e| format!("write {}: {e}", part.display()))?;
            Ok((bytes, hex(&hasher.finalize())))
        })();
        let (bytes, sha) = match result {
            Ok(v) => v,
            Err(e) => {
                let _ = fs::remove_file(&part);
                return Err(e);
            }
        };
        // Cancelled while the last read was in flight: install nothing.
        if cancel.load(Ordering::Relaxed) {
            let _ = fs::remove_file(&part);
            return Err("cancelled".into());
        }
        if let Some(want) = f.sha256 {
            if !want.eq_ignore_ascii_case(&sha) {
                let _ = fs::remove_file(&part);
                return Err(format!("{} does not match its published checksum; nothing was installed", f.name));
            }
        }
        fs::rename(&part, dir.join(f.name)).map_err(|e| format!("save {}: {e}", f.name))?;
        done_bytes += bytes;
        files.push(InstalledFile { name: f.name.to_string(), bytes, sha256: sha, published: f.sha256.is_some() });
    }
    let rec = Installed { files };
    let tmp = dir.join(format!(".installed.{}.tmp", ulid()));
    fs::write(&tmp, serde_json::to_vec_pretty(&rec).expect("serializable")).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dir.join("installed.json")).map_err(|e| e.to_string())?;
    Ok(rec)
}

/// Partial files a crash or an abandoned download left behind. One model
/// downloads at a time, and an abandoned transfer is flagged so it never
/// renames its file, so every partial file here is garbage. (On Windows a
/// file a stalled transfer still holds open is skipped until next time.)
fn sweep_parts(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') && name.ends_with(".part") {
            let _ = fs::remove_file(e.path());
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Remove a downloaded model's files. The project's transcriptions keep
/// naming the model they came from.
pub fn delete(app_data: &Path, id: &str) -> Result<(), String> {
    let dir = model_dir(app_data, id)?;
    match fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("delete {}: {e}", dir.display())),
    }
}

// ------------------------------------------------------------ the worker

/// Something that hears phones in a stretch of a WAV file. The real one
/// loads a model; tests supply their own.
pub trait Hearer: Send {
    fn hear(&mut self, wav: &Path, start_ms: f64, end_ms: f64) -> Result<Recognition, String>;
}

/// A downloaded model, loaded on first use.
pub struct ModelHearer {
    dir: PathBuf,
    spec: ModelSpec,
    loaded: Option<Recognizer>,
}

impl ModelHearer {
    pub fn new(app_data: &Path, spec: ModelSpec) -> Result<ModelHearer, String> {
        Ok(ModelHearer { dir: model_dir(app_data, spec.id)?, spec, loaded: None })
    }
}

impl Hearer for ModelHearer {
    fn hear(&mut self, wav: &Path, start_ms: f64, end_ms: f64) -> Result<Recognition, String> {
        if self.loaded.is_none() {
            let vocab_text = fs::read_to_string(self.dir.join(self.spec.vocab)).map_err(|e| format!("read the model's vocabulary: {e}"))?;
            let vocab = Vocab::from_hf_json(&vocab_text).map_err(|e| e.to_string())?;
            self.loaded = Some(Recognizer::load(&self.dir.join(self.spec.onnx), vocab, self.spec.normalize).map_err(|e| e.to_string())?);
        }
        let audio = wugs_asr::read_span(wav, start_ms, end_ms).map_err(|e| e.to_string())?;
        self.loaded.as_ref().expect("loaded above").recognize(&audio).map_err(|e| e.to_string())
    }
}

type Job = Box<dyn FnOnce(&mut HashMap<String, Box<dyn Hearer>>) + Send>;

/// The thread that owns loaded models. One job at a time, so two
/// suggestions never fight for the CPU or load a model twice.
pub struct Worker(mpsc::Sender<Job>);

impl Worker {
    pub fn spawn() -> Worker {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("wugs-assist".into())
            .spawn(move || {
                let mut hearers: HashMap<String, Box<dyn Hearer>> = HashMap::new();
                for job in rx {
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&mut hearers))).is_err() {
                        hearers.clear();
                    }
                }
            })
            .expect("spawn the assist thread");
        Worker(tx)
    }

    /// Hear a stretch of audio with model `id`, making its hearer with `make` the first time.
    pub fn hear(&self, id: &str, make: impl FnOnce() -> Result<Box<dyn Hearer>, String> + Send + 'static, wav: PathBuf, start_ms: f64, end_ms: f64) -> Result<Recognition, String> {
        let (reply, result) = mpsc::sync_channel(1);
        let id = id.to_string();
        self.0
            .send(Box::new(move |hearers| {
                let r = (|| {
                    if !hearers.contains_key(&id) {
                        hearers.insert(id.clone(), make()?);
                    }
                    hearers.get_mut(&id).expect("inserted above").hear(&wav, start_ms, end_ms)
                })();
                let _ = reply.send(r);
            }))
            .map_err(|_| "the speech assist thread has stopped".to_string())?;
        result.recv().map_err(|_| "the model failed unexpectedly; try again".to_string())?
    }

    /// Forget a model's loaded copy (it was deleted or replaced).
    pub fn forget(&self, id: &str) {
        let id = id.to_string();
        let _ = self.0.send(Box::new(move |hearers| {
            hearers.remove(&id);
        }));
    }
}

// ------------------------------------------------------------ downloads in flight

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadState {
    pub done: u64,
    pub total: u64,
    pub error: Option<String>,
}

/// Downloads in flight by model id: their state, cancel flag and a token.
/// Cancel removes the entry at once, so a transfer stalled in a read cannot
/// block a retry; the old thread updates nothing whose token is not its own.
#[derive(Default)]
pub struct Downloads(pub Mutex<HashMap<String, Inflight>>);

pub struct Inflight {
    pub state: DownloadState,
    pub cancel: Arc<AtomicBool>,
    pub token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRow {
    pub id: String,
    pub name: String,
    pub about: String,
    pub license: String,
    pub attribution: String,
    pub homepage: String,
    pub approx_mb: u32,
    pub installed: Option<Installed>,
    pub download: Option<DownloadState>,
}

pub fn rows(app_data: &Path, downloads: &Downloads) -> Vec<ModelRow> {
    let inflight = downloads.0.lock().expect("downloads lock");
    catalog()
        .into_iter()
        .map(|s| ModelRow {
            id: s.id.into(),
            name: s.name.into(),
            about: s.about.into(),
            license: s.license.into(),
            attribution: s.attribution.into(),
            homepage: s.homepage.into(),
            approx_mb: s.approx_mb,
            installed: installed(app_data, &s),
            download: inflight.get(s.id).map(|d| d.state.clone()),
        })
        .collect()
}

// ------------------------------------------------------------ session side

/// Where a suggestion's audio is, read on the session thread.
#[derive(Debug)]
pub struct Target {
    pub wav: PathBuf,
    pub start_ms: f64,
    pub end_ms: f64,
}

/// Check that `segment` can take a suggestion and find its audio in
/// `source`'s recording.
pub fn target(session: &mut crate::session::Session, text: &str, segment: &str, source: &str) -> Result<Target, String> {
    let p = crate::session::project(session)?;
    if p.proj.entity_kind(segment)?.as_deref() != Some("segment") || p.proj.field_str(segment, "text")?.as_deref() != Some(text) {
        return Err("That segment is not in the open text.".into());
    }
    if !p.proj.field_str(segment, "content")?.unwrap_or_default().trim().is_empty() {
        return Err("This segment already has a transcription; suggestions only fill empty segments.".into());
    }
    let (Some(start_ms), Some(end_ms)) = (p.proj.field_num(segment, "start")?, p.proj.field_num(segment, "end")?) else {
        return Err("This segment has no times on the recording.".into());
    };
    let blob = crate::session::audio_blob(p, source)?;
    let hex = wugs_blob::parse_ref(&blob).ok_or_else(|| format!("malformed blob ref {blob:?}"))?.to_string();
    let wav = p.root.join("blobs").join(hex);
    if !wav.is_file() {
        return Err("The recording's audio file is missing from this project folder.".into());
    }
    Ok(Target { wav, start_ms, end_ms })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggested {
    /// What was written; empty when the model heard no speech (nothing is written then).
    pub content: String,
    pub confidence: f64,
    pub notes: Vec<String>,
}

/// Write what the model heard into the segment, as the machine.
pub fn commit(session: &mut crate::session::Session, text: &str, segment: &str, model: &str, heard: &Recognition) -> Result<Suggested, String> {
    let content = heard.text();
    if content.is_empty() {
        return Ok(Suggested { content, confidence: 0.0, notes: vec!["The model heard no speech in this segment.".into()] });
    }
    let p = crate::session::project(session)?;
    let transcript = wugs_core::assist::MachineTranscript {
        text: content.clone(),
        model: model.to_string(),
        confidence: heard.confidence as f64,
        phones: heard.phones.iter().map(|ph| (ph.symbol.clone(), ph.confidence as f64)).collect(),
    };
    let res = p.machine_transcribe(text, segment, &transcript)?;
    Ok(Suggested { content, confidence: heard.confidence as f64, notes: crate::session::finding_lines(&res) })
}
