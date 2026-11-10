//! Praat analysis overlays (roadmap 46): pitch, intensity and the first
//! three formants, computed by the person's own Praat with `praat --run` and
//! drawn over the waveform by the Timeline.
//!
//! A recording is analysed in fixed 10 s chunks, each padded by 0.1 s so
//! frames near a chunk edge see the same signal as frames in the middle, and
//! each chunk is cached as `<project>/cache/praat-analysis/<blob hex>/<n>.tsv`
//! (one line per 10 ms frame: time, F0, intensity, F1, F2, F3; `--undefined--`
//! where Praat finds none). Panning back is free; only chunks never seen run
//! Praat. The settings are Praat's defaults for speech (pitch 75–600 Hz by
//! autocorrelation, intensity with a 75 Hz floor, Burg formants up to
//! 5500 Hz); they are part of the cache's folder name, so changing them later
//! never mixes results. Praat runs off the session thread.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;

use crate::praat::{praat_string, run_quietly, write_atomic};
use crate::session::{self, Session};

pub const CHUNK_MS: u32 = 10_000;
pub const STEP_MS: u32 = 10;
const PAD_S: f64 = 0.1;
/// Names the settings below; a change gets a fresh cache.
const SETTINGS: &str = "v1-p75-600-i75-f5500";
/// At most this many chunks per request (a minute of audio).
pub const MAX_CHUNKS: usize = 6;
const DEADLINE: Duration = Duration::from_secs(180);
/// One analysis at a time: a linguist panning through a long recording
/// must not start a Praat per pan.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// One chunk's frames, every `STEP_MS` from `start_ms + STEP_MS / 2`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub index: u32,
    pub start_ms: f64,
    pub step_ms: f64,
    pub pitch: Vec<Option<f32>>,
    pub intensity: Vec<Option<f32>>,
    pub f1: Vec<Option<f32>>,
    pub f2: Vec<Option<f32>>,
    pub f3: Vec<Option<f32>>,
}

/// Where a request's audio and cache are, found on the session thread.
pub struct Job {
    wav: PathBuf,
    dir: PathBuf,
}

pub fn job(session: &mut Session, source: &str) -> Result<Job, String> {
    let p = session::project(session)?;
    let blob = session::audio_blob(p, source)?;
    let hex = wugs_blob::parse_ref(&blob).ok_or_else(|| format!("malformed blob ref {blob:?}"))?.to_string();
    let wav = p.root.join("blobs").join(&hex);
    if !wav.is_file() {
        return Err("The recording's audio file is missing from this project folder.".into());
    }
    Ok(Job { wav, dir: p.root.join("cache").join("praat-analysis").join(hex).join(SETTINGS) })
}

fn chunk_file(dir: &Path, index: u32) -> PathBuf {
    dir.join(format!("{index}.tsv"))
}

/// The script that analyses `chunks` of `wav` into `dir`.
pub fn analysis_script(wav: &Path, dir: &Path, chunks: &[u32]) -> String {
    let mut s = String::from("# Written by wugs: pitch, intensity and formants for the Timeline.\n");
    s.push_str(&format!("long = Open long sound file: {}\ndur = Get total duration\n", praat_string(&wav.to_string_lossy())));
    for &n in chunks {
        let t0 = f64::from(n) * f64::from(CHUNK_MS) / 1000.0;
        let t1 = t0 + f64::from(CHUNK_MS) / 1000.0;
        let out = praat_string(&chunk_file(dir, n).to_string_lossy());
        s.push_str(&format!(
            "t0 = {t0}\n\
             t1 = min ({t1}, dur)\n\
             out$ = \"\"\n\
             if t1 - t0 > 0.1\n\
             \x20   selectObject: long\n\
             \x20   part = Extract part: max (0, t0 - {PAD_S}), min (dur, t1 + {PAD_S}), \"yes\"\n\
             \x20   pitch = To Pitch: 0, 75, 600\n\
             \x20   selectObject: part\n\
             \x20   intensity = To Intensity: 75, 0, \"yes\"\n\
             \x20   selectObject: part\n\
             \x20   formant = To Formant (burg): 0, 5, 5500, 0.025, 50\n\
             \x20   t = t0 + {half}\n\
             \x20   while t < t1\n\
             \x20       selectObject: pitch\n\
             \x20       f0 = Get value at time: t, \"Hertz\", \"linear\"\n\
             \x20       selectObject: intensity\n\
             \x20       db = Get value at time: t, \"cubic\"\n\
             \x20       selectObject: formant\n\
             \x20       f1 = Get value at time: 1, t, \"hertz\", \"linear\"\n\
             \x20       f2 = Get value at time: 2, t, \"hertz\", \"linear\"\n\
             \x20       f3 = Get value at time: 3, t, \"hertz\", \"linear\"\n\
             \x20       out$ = out$ + fixed$ (t, 3) + tab$ + fixed$ (f0, 1) + tab$ + fixed$ (db, 1) + tab$ + fixed$ (f1, 0) + tab$ + fixed$ (f2, 0) + tab$ + fixed$ (f3, 0) + newline$\n\
             \x20       t = t + {step}\n\
             \x20   endwhile\n\
             \x20   removeObject: part, pitch, intensity, formant\n\
             endif\n\
             writeFile: {out}, out$\n",
            half = f64::from(STEP_MS) / 2000.0,
            step = f64::from(STEP_MS) / 1000.0,
        ));
    }
    s.push_str("removeObject: long\n");
    s
}

/// Read one chunk's cache file. Each frame goes where its time column says
/// (a damaged line shifts nothing), and a comma decimal is read too.
pub fn parse_chunk(index: u32, text: &str) -> Chunk {
    let mut c = Chunk {
        index,
        start_ms: f64::from(index) * f64::from(CHUNK_MS),
        step_ms: f64::from(STEP_MS),
        pitch: Vec::new(),
        intensity: Vec::new(),
        f1: Vec::new(),
        f2: Vec::new(),
        f3: Vec::new(),
    };
    let num = |s: &str| s.trim().replace(',', ".").parse::<f32>().ok().filter(|v| v.is_finite());
    let per_chunk = (CHUNK_MS / STEP_MS) as usize;
    let mut rows: Vec<(usize, [Option<f32>; 5])> = Vec::new();
    for line in text.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        let Some(t) = cols.first().and_then(|t| num(t)) else { continue };
        if cols.len() != 6 {
            continue;
        }
        let at = (f64::from(t) * 1000.0 - c.start_ms - c.step_ms / 2.0) / c.step_ms;
        if !(-0.5..per_chunk as f64 - 0.5).contains(&at) {
            continue;
        }
        rows.push((at.round() as usize, [num(cols[1]), num(cols[2]), num(cols[3]), num(cols[4]), num(cols[5])]));
    }
    let len = rows.iter().map(|(i, _)| i + 1).max().unwrap_or(0);
    for v in [&mut c.pitch, &mut c.intensity, &mut c.f1, &mut c.f2, &mut c.f3] {
        v.resize(len, None);
    }
    for (i, [p, db, f1, f2, f3]) in rows {
        (c.pitch[i], c.intensity[i], c.f1[i], c.f2[i], c.f3[i]) = (p, db, f1, f2, f3);
    }
    c
}

/// The chunks asked for, running Praat once for those not cached yet.
pub fn analyse(praat: &Path, job: &Job, chunks: &[u32]) -> Result<Vec<Chunk>, String> {
    if chunks.len() > MAX_CHUNKS {
        return Err(format!("at most {MAX_CHUNKS} chunks at a time"));
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    // Runs never overlap (the lock above), so a scratch folder here is a run
    // the app did not finish (killed or crashed).
    if let Ok(entries) = fs::read_dir(&job.dir) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with("run-") {
                fs::remove_dir_all(e.path()).ok();
            }
        }
    }
    let missing: Vec<u32> = chunks.iter().copied().filter(|&n| !chunk_file(&job.dir, n).is_file()).collect();
    if !missing.is_empty() {
        fs::create_dir_all(&job.dir).map_err(|e| e.to_string())?;
        // Praat writes into a scratch folder, moved into place once whole, so
        // a killed run never leaves a short chunk in the cache.
        let scratch = job.dir.join(format!("run-{}", wugs_model::ulid()));
        fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
        let script = scratch.join("analyse.praat");
        let ran = write_atomic(&script, analysis_script(&job.wav, &scratch, &missing).as_bytes())
            .and_then(|()| run_quietly(Command::new(praat).arg("--run").arg(&script), DEADLINE).map_err(|e| format!("Praat could not analyse the recording: {e}")));
        // writeFile writes a chunk's file once, whole, so every file there is
        // complete even when a later chunk failed: keep them.
        for &n in &missing {
            let done = chunk_file(&scratch, n);
            if done.is_file() {
                fs::rename(&done, chunk_file(&job.dir, n)).ok();
            }
        }
        fs::remove_dir_all(&scratch).ok();
        ran?;
    }
    chunks
        .iter()
        .map(|&n| {
            let bytes = fs::read(chunk_file(&job.dir, n)).map_err(|_| "Praat did not write its analysis.".to_string())?;
            let text = wugs_legacy::decode(&bytes).map_err(|e| e.to_string())?;
            Ok(parse_chunk(n, &text))
        })
        .collect()
}
