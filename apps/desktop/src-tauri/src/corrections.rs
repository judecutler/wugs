//! Corrections export (roadmap 57, `docs/design/PACKAGES.md` §3): every
//! machine suggestion a person has checked, as a Kaldi-style folder of clips
//! with what the model heard beside what the person wrote. The folder runs
//! straight through `wugs-asr-eval` as a benchmark set, and serves as
//! fine-tuning data. Which suggestions may leave is the engine's rule
//! (`Project::corrections`: consent refusals and restricted access hold them
//! back); a suggestion with no consent record either way goes only when the
//! person exporting says they have the speakers' consent.
//!
//! The session thread only gathers what to write (`prepare`); cutting the
//! audio and writing files (`write`) runs off it.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use wugs_core::assist::Correction;

use crate::session::{self, Session};

/// What an export would hold, before anything is written.
#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Checked suggestions a consent record allows for AI use.
    pub consented: usize,
    /// Checked suggestions no consent record speaks for.
    pub unrecorded: usize,
    pub unchecked: usize,
    pub refused: usize,
    pub restricted: usize,
    pub unusable: usize,
}

/// One clip to cut and its lines in the folder's tables.
#[derive(Debug, Clone)]
pub struct Job {
    pub pair: Correction,
    pub wav: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    /// The folder written, inside the one the person chose.
    pub folder: String,
    pub written: usize,
    /// Clips that could not be cut, with why (the first few).
    pub problems: Vec<String>,
    pub failed: usize,
}

pub fn summary(s: &mut Session) -> Result<Summary, String> {
    let c = session::project(s)?.corrections()?;
    let consented = c.pairs.iter().filter(|p| p.consented).count();
    Ok(Summary { consented, unrecorded: c.pairs.len() - consented, unchecked: c.unchecked, refused: c.refused, restricted: c.restricted, unusable: c.unusable })
}

/// The pairs to export and where each one's recording is. `unrecorded`
/// is the person's word that they have consent where no record says.
/// A pair whose recording has no audio file here is left out, counted in
/// the summary's `unusable`.
pub fn prepare(s: &mut Session, unrecorded: bool) -> Result<(Vec<Job>, Summary), String> {
    let mut sum = summary(s)?;
    let p = session::project(s)?;
    let mut jobs = Vec::new();
    for pair in p.corrections()?.pairs {
        if !pair.consented && !unrecorded {
            continue;
        }
        let wav = session::audio_blob(p, &pair.source)
            .ok()
            .and_then(|b| wugs_blob::parse_ref(&b).map(|h| p.root.join("blobs").join(h)))
            .filter(|w| w.is_file());
        match wav {
            Some(wav) => jobs.push(Job { pair, wav }),
            None => {
                if pair.consented { sum.consented -= 1 } else { sum.unrecorded -= 1 }
                sum.unusable += 1;
            }
        }
    }
    Ok((jobs, sum))
}

/// Write the folder: `wav/<segment>.wav` (16 kHz mono PCM16), `wav.scp`,
/// `text.good` (the person's transcription), `text.machine`, `utt2model`,
/// `utt2confidence`, `retimed` and a README datasheet. `today` names the
/// folder and dates the datasheet (`YYYY-MM-DD`).
pub fn write(jobs: &[Job], sum: &Summary, unrecorded: bool, parent: &Path, today: &str) -> Result<Exported, String> {
    if jobs.is_empty() {
        return Err("There are no checked suggestions to export.".into());
    }
    let mut folder = parent.join(format!("wugs-corrections-{today}"));
    let mut n = 2;
    while folder.exists() {
        folder = parent.join(format!("wugs-corrections-{today}-{n}"));
        n += 1;
    }
    fs::create_dir_all(folder.join("wav")).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut tables: BTreeMap<&str, String> = BTreeMap::new();
    let (mut written, mut failed, mut problems) = (0, 0, Vec::new());
    let mut models: BTreeMap<String, usize> = BTreeMap::new();
    let (mut consented, mut retimed) = (0, 0);
    for job in jobs {
        let p = &job.pair;
        let utt = &p.segment;
        let clip = folder.join("wav").join(format!("{utt}.wav"));
        let cut = wugs_asr::read_span(&job.wav, p.start_ms, p.end_ms).map_err(|e| e.to_string()).and_then(|audio| write_wav(&clip, &audio));
        if let Err(e) = cut {
            failed += 1;
            if problems.len() < 3 {
                problems.push(format!("A segment at {:.1} s: {e}", p.start_ms / 1000.0));
            }
            continue;
        }
        written += 1;
        let one_line = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
        for (table, value) in [
            ("wav.scp", format!("wav/{utt}.wav")),
            ("text.good", one_line(&p.person)),
            ("text.machine", one_line(&p.machine)),
            ("utt2model", p.model.clone()),
            ("utt2confidence", format!("{:.3}", p.confidence)),
        ] {
            let t = tables.entry(table).or_default();
            t.push_str(&format!("{utt} {value}\n"));
        }
        if p.retimed {
            retimed += 1;
            tables.entry("retimed").or_default().push_str(&format!("{utt}\n"));
        }
        if p.consented {
            consented += 1;
        }
        *models.entry(p.model.clone()).or_default() += 1;
    }
    if written == 0 {
        fs::remove_dir_all(&folder).ok();
        return Err(format!("No clip could be cut. {}", problems.join(" ")));
    }
    for (name, body) in &tables {
        fs::write(folder.join(name), body).map_err(|e| format!("{name}: {e}"))?;
    }
    let readme = datasheet(today, written, consented, unrecorded, retimed, &models, sum);
    fs::write(folder.join("README.md"), readme).map_err(|e| format!("README.md: {e}"))?;
    Ok(Exported { folder: folder.display().to_string(), written, problems, failed })
}

fn datasheet(today: &str, written: usize, consented: usize, unrecorded: bool, retimed: usize, models: &BTreeMap<String, usize>, sum: &Summary) -> String {
    let models = models.iter().map(|(m, n)| format!("- `{m}`: {n}")).collect::<Vec<_>>().join("\n");
    let basis = if unrecorded && consented < written {
        format!("{consented} come from recordings whose consent record allows AI use. For the other {}, no consent record says either way, and the person exporting confirmed they have the speakers' consent to use them for training and testing speech models.", written - consented)
    } else {
        "Every clip comes from a recording whose consent record allows AI use.".to_string()
    };
    format!(
        "# Corrected machine transcriptions (wugs)\n\n\
Exported {today}. {written} clips, each a segment where a phone model suggested a transcription and a person then checked and corrected it in wugs.\n\n\
**These are speech recordings of real people. Treat this folder as you would the recordings themselves:** share it only as far as the speakers agreed.\n\n\
## Consent\n\n{basis} Held back from this export: {} refused by a consent record, {} with a restricted or community access level, {} not yet checked by a person, {} without a recording or times.\n\n\
## Files\n\n\
Kaldi-style tables, one line per clip, keyed by the segment's id:\n\n\
- `wav/<id>.wav`: the segment's audio, 16 kHz mono 16-bit PCM\n\
- `wav.scp`: id and clip path\n\
- `text.good`: the person's transcription (the reference)\n\
- `text.machine`: what the model heard (IPA phones, spaces at pauses)\n\
- `utt2model`, `utt2confidence`: which model suggested it and how sure it was (mean phone confidence, 0 to 1)\n\
- `retimed`: clips whose times were changed after the suggestion, so the model heard a slightly different span ({retimed} here)\n\n\
The person's transcription follows the project's own conventions: it may be phonetic, phonemic or orthographic, which matters when it is compared with phones.\n\n\
## Models\n\n{models}\n\n\
## Scoring\n\n\
The folder is a test set for `wugs-asr-eval` (in the wugs repository):\n\n\
```\nwugs-asr-eval --runtime <runtime> --model <model.onnx> --vocab <vocab> --set <this folder> --out predictions.jsonl\n```\n",
        sum.refused, sum.restricted, sum.unchecked, sum.unusable,
    )
}

fn write_wav(path: &Path, audio: &[f32]) -> Result<(), String> {
    let data = (audio.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + audio.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    let rate = wugs_asr::SAMPLE_RATE;
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in audio {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    let mut f = fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(&out).map_err(|e| e.to_string())
}

/// Today's date in UTC as `YYYY-MM-DD` (civil-from-days, Hinnant).
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}
