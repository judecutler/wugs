use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use wugs_asr::{Phone, Recognition};
use wugs_desktop::assist::{self, Hearer, Worker};
use wugs_package::{File, Manifest, Model};
use wugs_desktop::session::{self, Session};
use wugs_model::ulid;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let d = std::env::temp_dir().join(format!("wugs-assist-test-{}", ulid()));
        fs::create_dir_all(&d).unwrap();
        TempDir(d)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

/// Serves each file once per request, by path, from a local port.
fn serve(files: Vec<(&'static str, Vec<u8>)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut req = [0u8; 2048];
            let n = s.read(&mut req).unwrap_or(0);
            let line = String::from_utf8_lossy(&req[..n]).lines().next().unwrap_or("").to_string();
            let path = line.split_whitespace().nth(1).unwrap_or("/").trim_start_matches('/').to_string();
            match files.iter().find(|(name, _)| *name == path) {
                Some((_, body)) => {
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = s.write_all(body);
                }
                None => {
                    let _ = write!(s, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
            }
        }
    });
    base
}

fn spec(base: &str, sha: Option<&'static str>) -> Manifest {
    let file = |path: &str, sha256: Option<&str>| File { path: path.into(), url: format!("{base}/{path}"), sha256: sha256.map(str::to_string), bytes: None };
    Manifest {
        id: "test-model".into(),
        source: "hf:test/test".into(),
        version: "1".into(),
        name: "Test".into(),
        about: String::new(),
        license: "Apache-2.0".into(),
        attribution: String::new(),
        homepage: String::new(),
        approx_mb: Some(1),
        model: Model { capability: "phone-transcription".into(), runtime: "onnx-ctc-raw".into(), languages: vec!["*".into()], params: Default::default() },
        files: vec![file("model.onnx", sha), file("vocab.json", None)],
        evaluation: None,
    }
}

const MODEL: &[u8] = b"not really onnx, but bytes to hash";
const MODEL_SHA: &str = "6d3b3f0fe7c6ff94d7bd1bc5c5d0e1f8fc9b7f2fbde2c48fe8f3d5e6bfb0d2a1";

fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn a_download_installs_every_file_and_records_their_hashes() {
    let dir = TempDir::new();
    let base = serve(vec![("model.onnx", MODEL.to_vec()), ("vocab.json", b"{}".to_vec())]);
    let s = spec(&base, None);
    assert_eq!(assist::installed(&dir.0, &s), None);
    let seen = std::sync::Mutex::new(Vec::new());
    let rec = assist::download(&dir.0, &s, &AtomicBool::new(false), &|d, t| seen.lock().unwrap().push((d, t))).unwrap();
    assert_eq!(rec.files[0].sha256, sha256(MODEL));
    assert!(!rec.files[0].published, "no published hash: recorded at download");
    assert_eq!(assist::installed(&dir.0, &s), Some(rec));
    let last = *seen.lock().unwrap().last().unwrap();
    assert_eq!(last.0, MODEL.len() as u64 + 2);
    assert_eq!(fs::read(dir.0.join("models/test-model/model.onnx")).unwrap(), MODEL);
    let leftovers: Vec<_> = fs::read_dir(dir.0.join("models/test-model")).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).filter(|n| n.to_string_lossy().starts_with('.')).collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn a_published_checksum_must_match() {
    let dir = TempDir::new();
    let base = serve(vec![("model.onnx", MODEL.to_vec()), ("vocab.json", b"{}".to_vec())]);
    let err = assist::download(&dir.0, &spec(&base, Some(MODEL_SHA)), &AtomicBool::new(false), &|_, _| {}).unwrap_err();
    assert!(err.contains("checksum"), "{err}");
    assert!(!dir.0.join("models/test-model/model.onnx").exists());
    assert_eq!(assist::installed(&dir.0, &spec(&base, None)), None);

    let good: &'static str = Box::leak(sha256(MODEL).into_boxed_str());
    let rec = assist::download(&dir.0, &spec(&base, Some(good)), &AtomicBool::new(false), &|_, _| {}).unwrap();
    assert!(rec.files[0].published);
}

#[test]
fn a_published_size_must_match() {
    let dir = TempDir::new();
    let base = serve(vec![("model.onnx", MODEL.to_vec()), ("vocab.json", b"{}".to_vec())]);
    let mut s = spec(&base, None);
    s.files[0].bytes = Some(MODEL.len() as u64 + 1);
    let err = assist::download(&dir.0, &s, &AtomicBool::new(false), &|_, _| {}).unwrap_err();
    assert!(err.contains("size"), "{err}");
    assert_eq!(assist::installed(&dir.0, &s), None);
    s.files[0].bytes = Some(MODEL.len() as u64);
    assist::download(&dir.0, &s, &AtomicBool::new(false), &|_, _| {}).unwrap();
}

#[test]
fn a_cancelled_or_failed_download_leaves_nothing_installed() {
    let dir = TempDir::new();
    let base = serve(vec![("model.onnx", MODEL.to_vec())]);
    let err = assist::download(&dir.0, &spec(&base, None), &AtomicBool::new(true), &|_, _| {}).unwrap_err();
    assert_eq!(err, "cancelled");
    let err = assist::download(&dir.0, &spec(&base, None), &AtomicBool::new(false), &|_, _| {}).unwrap_err();
    assert!(err.contains("vocab.json"), "the missing second file fails it: {err}");
    assert_eq!(assist::installed(&dir.0, &spec(&base, None)), None);
    let parts = fs::read_dir(dir.0.join("models/test-model")).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".part")).count();
    assert_eq!(parts, 0);
}

#[test]
fn a_new_download_sweeps_what_a_crash_left() {
    let dir = TempDir::new();
    let base = serve(vec![("model.onnx", MODEL.to_vec()), ("vocab.json", b"{}".to_vec())]);
    let model_dir = dir.0.join("models/test-model");
    fs::create_dir_all(&model_dir).unwrap();
    fs::write(model_dir.join(".model.onnx.01OLD.part"), vec![0u8; 1024]).unwrap();
    assist::download(&dir.0, &spec(&base, None), &AtomicBool::new(false), &|_, _| {}).unwrap();
    assert!(!model_dir.join(".model.onnx.01OLD.part").exists());
}

#[test]
fn only_https_or_this_machine() {
    let dir = TempDir::new();
    let mut s = spec("http://example.org", None);
    s.files.truncate(1);
    let err = assist::download(&dir.0, &s, &AtomicBool::new(false), &|_, _| {}).unwrap_err();
    assert!(err.contains("insecure"), "{err}");
    assert!(wugs_desktop::capability::catalogue().iter().all(|m| m.files.iter().all(|f| f.url.starts_with("https://"))));
}

#[test]
fn deleting_removes_the_files_and_bad_ids_touch_nothing() {
    let dir = TempDir::new();
    let base = serve(vec![("model.onnx", MODEL.to_vec()), ("vocab.json", b"{}".to_vec())]);
    assist::download(&dir.0, &spec(&base, None), &AtomicBool::new(false), &|_, _| {}).unwrap();
    assist::delete(&dir.0, "test-model").unwrap();
    assert!(!dir.0.join("models/test-model").exists());
    assist::delete(&dir.0, "test-model").unwrap();
    for bad in ["..", "../x", "a/b", ""] {
        assert!(assist::delete(&dir.0, bad).is_err(), "{bad}");
    }
}

// ------------------------------------------------------------ suggest

struct Fixed(Recognition);

impl Hearer for Fixed {
    fn hear(&mut self, wav: &Path, start_ms: f64, end_ms: f64) -> Result<Recognition, String> {
        assert!(wav.is_file());
        assert!(end_ms > start_ms);
        Ok(self.0.clone())
    }
}

fn phone(s: &str, a: f64, c: f32) -> Phone {
    Phone { symbol: s.into(), start_ms: a, end_ms: a + 40.0, confidence: c }
}

fn wav(path: &Path, seconds: u32) {
    let rate = 16_000u32;
    let frames = rate * seconds;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + frames * 2).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(frames * 2).to_le_bytes());
    b.resize(b.len() + frames as usize * 2, 0);
    fs::write(path, b).unwrap();
}

/// A project with a recording and a text with one empty timed segment.
fn project() -> (TempDir, Session, String, String, String) {
    let dir = TempDir::new();
    let mut s: Session = None;
    session::create_project(&mut s, dir.0.join("p").to_str().unwrap(), "prs_Ana").unwrap();
    let file = dir.0.join("rec.wav");
    wav(&file, 3);
    let source = session::import_media(&mut s, file.to_str().unwrap()).unwrap().id;
    let text = session::create_text(&mut s, "Story").unwrap();
    let seg = session::add_segment(&mut s, &text, None, Some(500.0), Some(1500.0), "").unwrap().id;
    (dir, s, source, text, seg)
}

#[test]
fn a_suggestion_is_heard_and_written_as_the_machine() {
    let (_dir, mut s, source, text, seg) = project();
    let target = assist::target(&mut s, &text, &seg, &source).unwrap();
    assert_eq!((target.start_ms, target.end_ms), (500.0, 1500.0));
    let worker = Worker::spawn();
    let heard = Recognition { phones: vec![phone("k", 0.0, 0.9), phone("o", 40.0, 0.3), phone("p", 400.0, 0.6)], confidence: 0.6 };
    let fixed = heard.clone();
    let got = worker.hear("m", move || Ok(Box::new(Fixed(fixed)) as Box<dyn Hearer>), target.wav, target.start_ms, target.end_ms).unwrap();
    let done = assist::commit(&mut s, &text, &seg, &spec("http://127.0.0.1:1", None), &got).unwrap();
    assert_eq!(done.content, "ko p");
    let row = session::get_text(&mut s, &text).unwrap().segments.into_iter().find(|r| r.id == seg).unwrap();
    assert_eq!(row.content, "ko p");
    let mark = row.machine.expect("unverified");
    assert_eq!(mark.model, "test-model");
    assert!((mark.confidence - 0.6).abs() < 1e-6);

    // Edited by a person: verified, the tint goes.
    session::edit_segment_content(&mut s, &text, &seg, "kop").unwrap();
    let row = session::get_text(&mut s, &text).unwrap().segments.into_iter().find(|r| r.id == seg).unwrap();
    assert!(row.machine.is_none());
    // And a filled segment takes no suggestion.
    assert!(assist::target(&mut s, &text, &seg, &source).unwrap_err().contains("already has a transcription"));
}

#[test]
fn hearing_nothing_writes_nothing() {
    let (_dir, mut s, _source, text, seg) = project();
    let done = assist::commit(&mut s, &text, &seg, &spec("http://127.0.0.1:1", None), &Recognition { phones: vec![], confidence: 0.0 }).unwrap();
    assert_eq!(done.content, "");
    assert!(!done.notes.is_empty());
    let row = session::get_text(&mut s, &text).unwrap().segments.into_iter().find(|r| r.id == seg).unwrap();
    assert_eq!(row.content, "");
    assert!(row.machine.is_none());
}

#[test]
fn targets_are_checked() {
    let (_dir, mut s, source, text, seg) = project();
    let other = session::create_text(&mut s, "Other").unwrap();
    assert!(assist::target(&mut s, &other, &seg, &source).is_err(), "segment of another text");
    let untimed = session::add_segment(&mut s, &text, None, None, None, "").unwrap().id;
    assert!(assist::target(&mut s, &text, &untimed, &source).unwrap_err().contains("no times"));
    assert!(assist::target(&mut s, &text, &seg, "src_nope").is_err());
}

#[test]
fn the_worker_survives_a_failing_model() {
    struct Broken;
    impl Hearer for Broken {
        fn hear(&mut self, _: &Path, _: f64, _: f64) -> Result<Recognition, String> {
            panic!("model blew up")
        }
    }
    let worker = Worker::spawn();
    let err = worker.hear("b", || Ok(Box::new(Broken) as Box<dyn Hearer>), PathBuf::from("x.wav"), 0.0, 1.0).unwrap_err();
    assert!(err.contains("try again"), "{err}");
    let ok = worker.hear("m", || Ok(Box::new(Fixed(Recognition { phones: vec![], confidence: 0.0 })) as Box<dyn Hearer>), std::env::current_exe().unwrap(), 0.0, 1.0);
    assert!(ok.is_ok(), "{ok:?}");
}

#[test]
fn the_fbank_runtime_hears_through_the_catalogue_adapter() {
    // A tiny random Zipformer (wugs-asr's fixture) installed under the
    // ZIPA manifest: the adapter reads the manifest's [runtime] files.
    let dir = TempDir::new();
    let m = wugs_desktop::capability::manifest("zipa-cr-small").unwrap();
    let models = dir.0.join("models").join(&m.id);
    fs::create_dir_all(&models).unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../crates/wugs-asr/tests/fixtures");
    fs::copy(fixtures.join("tiny-zipformer.int8.onnx"), models.join(m.param_file("onnx").unwrap())).unwrap();
    let mut tokens = String::from("<blk> 0\n<sos/eos> 1\n<unk> 2\n▁ 3\n");
    for i in 4..127 {
        tokens.push_str(&format!("p{i} {i}\n"));
    }
    fs::write(models.join(m.param_file("tokens").unwrap()), tokens).unwrap();
    let file = dir.0.join("rec.wav");
    wav(&file, 2);
    let mut h = assist::hearer(&dir.0, m).unwrap();
    let heard = h.hear(&file, 200.0, 1700.0).unwrap();
    assert!(heard.phones.iter().all(|p| p.end_ms <= 1500.0 + 1e-6));
}
