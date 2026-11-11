//! Transcribe a Kaldi-style phone test set with one model, the way wugs runs
//! it, for scoring (roadmap 52, `docs/design/PACKAGES.md` §3).
//!
//! ```text
//! wugs-asr-eval --runtime onnx-ctc-fbank --model model.int8.onnx --vocab tokens.txt \
//!     --set <dir with wav.scp, text.good and a language file> --out predictions.jsonl [--limit N]
//! ```
//!
//! Each output line is one utterance in the prediction format PRiSM's
//! scorer reads (`{"<utt>": {"pred": [{"processed_transcript": …}],
//! "passthrough": {"utt_id", "target", "lang_sym"}}}`), so the published
//! PFER is computed by the benchmark's own code. A summary on stderr gives
//! the audio seconds, processing seconds, real-time factor and peak memory.
//! `wav.scp` entries must be plain WAV paths (relative to the set); Kaldi
//! pipe commands are refused. Utterances longer than the 30 s limit are
//! skipped and counted.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use wugs_asr::fbank_ctc::FbankRecognizer;
use wugs_asr::{read_span, Recognition, Recognizer, Vocab, MAX_SECONDS};

enum Model {
    Raw(Recognizer),
    Fbank(FbankRecognizer),
}

impl Model {
    fn recognize(&self, audio: &[f32]) -> Result<Recognition, wugs_asr::Error> {
        match self {
            Model::Raw(r) => r.recognize(audio),
            Model::Fbank(r) => r.recognize(audio),
        }
    }
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn table(path: &Path) -> Result<Vec<(String, String)>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| match l.split_once(char::is_whitespace) {
            Some((k, v)) => (k.to_string(), v.trim().to_string()),
            None => (l.trim().to_string(), String::new()),
        })
        .collect())
}

fn peak_mb() -> Option<f64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let kb: f64 = status.lines().find(|l| l.starts_with("VmHWM:"))?.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let need = |n: &str| arg(&args, n).ok_or_else(|| format!("missing {n}"));
    let runtime = need("--runtime")?;
    let model_path = PathBuf::from(need("--model")?);
    let vocab_text = fs::read_to_string(need("--vocab")?).map_err(|e| e.to_string())?;
    let set = PathBuf::from(need("--set")?);
    let out = PathBuf::from(need("--out")?);
    let limit: usize = arg(&args, "--limit").map(|v| v.parse().map_err(|_| "--limit takes a number".to_string())).transpose()?.unwrap_or(usize::MAX);

    let t0 = Instant::now();
    let model = match runtime.as_str() {
        "onnx-ctc-raw" => Model::Raw(Recognizer::load(&model_path, Vocab::from_hf_json(&vocab_text).map_err(|e| e.to_string())?, args.iter().any(|a| a == "--normalize")).map_err(|e| e.to_string())?),
        "onnx-ctc-fbank" => Model::Fbank(FbankRecognizer::load(&model_path, Vocab::from_tokens_txt(&vocab_text).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?),
        other => return Err(format!("unknown runtime {other:?}")),
    };
    eprintln!("loaded in {:.2?}", t0.elapsed());

    let wavs = table(&set.join("wav.scp"))?;
    let targets: HashMap<String, String> = table(&set.join("text.good"))?.into_iter().collect();
    let langs: HashMap<String, String> = ["text.lang", "text"].iter().map(|f| set.join(f)).find(|p| p.is_file()).map(|p| table(&p)).transpose()?.unwrap_or_default().into_iter().collect();

    let mut file = fs::File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let (mut audio_s, mut work_s, mut done, mut skipped) = (0.0f64, 0.0f64, 0usize, 0usize);
    for (utt, wav) in wavs.into_iter().take(limit) {
        if wav.ends_with('|') || wav.contains(' ') {
            return Err(format!("{utt}: wav.scp entries must be plain WAV paths, not commands ({wav})"));
        }
        let Some(target) = targets.get(&utt) else { continue };
        let path = if Path::new(&wav).is_absolute() { PathBuf::from(&wav) } else { set.join(&wav) };
        let audio = read_span(&path, 0.0, MAX_SECONDS * 1000.0).map_err(|e| format!("{utt}: {e}"))?;
        let seconds = audio.len() as f64 / wugs_asr::SAMPLE_RATE as f64;
        // A full 30 s back means the recording may go on: wugs would refuse it.
        if seconds >= MAX_SECONDS - 0.01 {
            skipped += 1;
            continue;
        }
        let t = Instant::now();
        let heard = match model.recognize(&audio) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{utt}: {e}");
                skipped += 1;
                continue;
            }
        };
        work_s += t.elapsed().as_secs_f64();
        audio_s += seconds;
        done += 1;
        let prediction: String = heard.phones.iter().map(|p| p.symbol.as_str()).collect::<Vec<_>>().join(" ");
        let line = serde_json::json!({ utt.clone(): {
            "pred": [{ "processed_transcript": prediction }],
            "passthrough": { "utt_id": utt, "target": target, "lang_sym": langs.get(&utt).cloned().unwrap_or_else(|| "combined".into()) },
        }});
        writeln!(file, "{line}").map_err(|e| e.to_string())?;
    }
    eprintln!(
        "{done} utterances, {skipped} skipped; {audio_s:.1} s of audio in {work_s:.1} s (real-time factor {:.3}); peak memory {} MB",
        if audio_s > 0.0 { work_s / audio_s } else { 0.0 },
        peak_mb().map_or("?".into(), |m| format!("{m:.0}"))
    );
    Ok(())
}
