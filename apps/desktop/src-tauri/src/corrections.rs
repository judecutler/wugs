//! Corrections export (roadmap 57, `docs/design/PACKAGES.md` §3): every
//! machine suggestion a person has checked, as a Kaldi-style folder of clips
//! with what the model heard beside what the person wrote. The folder runs
//! straight through `wugs-asr-eval` as a benchmark set, and serves as
//! fine-tuning data.
//!
//! wugs is a tool, not an ethics board (owner, 2026-11-10): consent records
//! and access levels the linguist entered are shown and set the defaults
//! (clips a record says no to, or with a restricted level, start left out),
//! and the person decides; the datasheet records what the records say for
//! every clip, so a review board has the facts.
//!
//! The session thread only gathers what to write (`prepare`); cutting the
//! audio and writing files (`write`) runs off it.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use wugs_core::assist::{Consent, Correction, Corrections};

use crate::session::{self, Session};

/// What an export would hold, before anything is written.
#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Checked suggestions nothing in the project's records speaks against.
    pub ready: usize,
    /// Checked suggestions a consent record says no to (for AI use).
    pub recorded_no: usize,
    /// Checked suggestions with a restricted or community access level (and no recorded no).
    pub restricted: usize,
    pub unchecked: usize,
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

/// A pair the project's own records speak against: a recorded no, or a
/// restricted access level. Left out unless the person includes them.
fn flagged(p: &Correction) -> bool {
    p.consent == Consent::No || p.restricted
}

pub fn summary(s: &mut Session) -> Result<Summary, String> {
    Ok(tally(&session::project(s)?.corrections()?))
}

fn tally(c: &Corrections) -> Summary {
    let mut sum = Summary { unchecked: c.unchecked, unusable: c.unusable, ..Summary::default() };
    for p in &c.pairs {
        count(&mut sum, p, 1);
    }
    sum
}

fn count(sum: &mut Summary, p: &Correction, by: isize) {
    let slot = if p.consent == Consent::No {
        &mut sum.recorded_no
    } else if p.restricted {
        &mut sum.restricted
    } else {
        &mut sum.ready
    };
    *slot = slot.saturating_add_signed(by);
}

/// The pairs to export and where each one's recording is. `flagged`: the
/// person also includes pairs their records speak against. A pair whose
/// recording has no audio file here is left out, counted as `unusable`.
pub fn prepare(s: &mut Session, include_flagged: bool) -> Result<(Vec<Job>, Summary), String> {
    let p = session::project(s)?;
    let c = p.corrections()?;
    let mut sum = tally(&c);
    let mut jobs = Vec::new();
    for pair in c.pairs {
        if flagged(&pair) && !include_flagged {
            continue;
        }
        let wav = session::audio_blob(p, &pair.source)
            .ok()
            .and_then(|b| wugs_blob::parse_ref(&b).map(|h| p.root.join("blobs").join(h)))
            .filter(|w| w.is_file());
        match wav {
            Some(wav) => jobs.push(Job { pair, wav }),
            None => {
                count(&mut sum, &pair, -1);
                sum.unusable += 1;
            }
        }
    }
    Ok((jobs, sum))
}

/// Write the folder: `wav/<segment>.wav` (16 kHz mono PCM16), `wav.scp`,
/// `text.good` (the person's transcription), `text.machine`, `utt2model`,
/// `utt2confidence`, `utt2consent` (what the project's consent records say),
/// `restricted`, `retimed` and a README datasheet. `today` names the folder
/// and dates the datasheet (`YYYY-MM-DD`).
pub fn write(jobs: &[Job], sum: &Summary, parent: &Path, today: &str) -> Result<Exported, String> {
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
    let mut tally = Tally::default();
    for job in jobs {
        let p = &job.pair;
        let utt = &p.segment;
        // Ids name files and key whitespace-separated tables; a synced id is
        // untrusted, so anything but letters, digits and `_` is refused.
        if utt.is_empty() || !utt.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            failed += 1;
            if problems.len() < 3 {
                problems.push(format!("A segment with a malformed id ({:?}) was left out.", utt.chars().take(40).collect::<String>()));
            }
            continue;
        }
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
            ("utt2consent", p.consent.as_str().to_string()),
        ] {
            let t = tables.entry(table).or_default();
            t.push_str(&format!("{utt} {value}\n"));
        }
        if p.retimed {
            tally.retimed += 1;
            tables.entry("retimed").or_default().push_str(&format!("{utt}\n"));
        }
        if p.restricted {
            tally.restricted += 1;
            tables.entry("restricted").or_default().push_str(&format!("{utt}\n"));
        }
        match p.consent {
            Consent::Yes => tally.yes += 1,
            Consent::Unrecorded => tally.unrecorded += 1,
            Consent::No => tally.no += 1,
        }
        *models.entry(p.model.clone()).or_default() += 1;
    }
    if written == 0 {
        fs::remove_dir_all(&folder).ok();
        return Err(format!("No clip could be cut. {}", problems.join(" ")));
    }
    let readme = datasheet(today, written, &tally, &models, sum, jobs.iter().any(|j| flagged(&j.pair)));
    let finish = || -> Result<(), String> {
        for (name, body) in &tables {
            fs::write(folder.join(name), body).map_err(|e| format!("{name}: {e}"))?;
        }
        fs::write(folder.join("README.md"), &readme).map_err(|e| format!("README.md: {e}"))
    };
    // A folder without its tables is not a set; leave nothing half written.
    if let Err(e) = finish() {
        fs::remove_dir_all(&folder).ok();
        return Err(e);
    }
    Ok(Exported { folder: folder.display().to_string(), written, problems, failed })
}

/// What the written clips' records say.
#[derive(Default)]
struct Tally {
    yes: usize,
    unrecorded: usize,
    no: usize,
    restricted: usize,
    retimed: usize,
}

fn datasheet(today: &str, written: usize, t: &Tally, models: &BTreeMap<String, usize>, sum: &Summary, included_flagged: bool) -> String {
    let models = models.iter().map(|(m, n)| format!("- `{m}`: {n}")).collect::<Vec<_>>().join("\n");
    let left_out = if included_flagged { String::new() } else if sum.recorded_no + sum.restricted > 0 {
        format!(" Left out by the exporter's choice: {} clips a consent record says no to and {} with a restricted or community access level.", sum.recorded_no, sum.restricted)
    } else {
        String::new()
    };
    let against = if t.no + t.restricted > 0 {
        format!(" The exporter chose to include {} clips a consent record in the project says no to and {} with a restricted or community access level (both listed: `utt2consent`, `restricted`).", t.no, t.restricted)
    } else {
        String::new()
    };
    format!(
        "# Corrected machine transcriptions (wugs)\n\n\
Exported {today}. {written} clips, each a segment where a phone model suggested a transcription and a person then checked and corrected it in wugs.\n\n\
**These are speech recordings of real people. Treat this folder as you would the recordings themselves:** share it only as far as the speakers agreed.\n\n\
## What the project's records say\n\n\
wugs records consent as the project enters it and does not judge it; the people who made this export, and their review board, are responsible for its use. Of these clips, {} are covered by a consent record that allows AI use, {} by none that speaks about it, and {} by one that does not allow it.{against}{left_out} Not in this export: {} suggestions not yet checked by a person, {} without a recording or times.\n\n\
## Files\n\n\
Kaldi-style tables, one line per clip, keyed by the segment's id:\n\n\
- `wav/<id>.wav`: the segment's audio, 16 kHz mono 16-bit PCM\n\
- `wav.scp`: id and clip path\n\
- `text.good`: the person's transcription (the reference)\n\
- `text.machine`: what the model heard (IPA phones, spaces at pauses)\n\
- `utt2model`, `utt2confidence`: which model suggested it and how sure it was (mean phone confidence, 0 to 1)\n\
- `utt2consent`: what the project's consent records say about AI use of the clip (`recorded-yes`, `not-recorded`, `recorded-no`)\n\
- `restricted`: clips whose text or recording has a restricted or community access level ({} here)\n\
- `retimed`: clips whose times were changed after the suggestion, so the model heard a slightly different span ({} here)\n\n\
The person's transcription follows the project's own conventions: it may be phonetic, phonemic or orthographic, which matters when it is compared with phones.\n\n\
## Models\n\n{models}\n\n\
## Scoring\n\n\
The folder is a test set for `wugs-asr-eval` (in the wugs repository):\n\n\
```\nwugs-asr-eval --runtime <runtime> --model <model.onnx> --vocab <vocab> --set <this folder> --out predictions.jsonl\n```\n",
        t.yes, t.unrecorded, t.no, sum.unchecked, sum.unusable, t.restricted, t.retimed,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A project whose one checked suggestion sits on a restricted recording.
    fn restricted_pair(dir: &Path) -> Session {
        let mut s: Session = None;
        session::create_project(&mut s, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let wav = dir.join("rec.wav");
        write_wav(&wav, &vec![0.0; 32_000]).unwrap();
        let source = session::import_media(&mut s, wav.to_str().unwrap()).unwrap().id;
        let text = session::create_text(&mut s, "Story").unwrap();
        let seg = session::add_segment(&mut s, &text, None, Some(0.0), Some(1000.0), "").unwrap().id;
        let p = session::project(&mut s).unwrap();
        let heard = wugs_core::assist::MachineTranscript { text: "ko".into(), model: "m".into(), confidence: 0.5, phones: vec![], source: Some(source.clone()) };
        p.machine_transcribe(&text, &seg, &heard).unwrap();
        p.edit_content(&text, &seg, "kɔ").unwrap();
        p.set_access(&source, "restricted").unwrap();
        s
    }

    #[test]
    fn what_the_records_speak_against_starts_out_and_goes_in_on_request() {
        let dir = std::env::temp_dir().join(format!("wugs-corrections-{}", wugs_model::ulid()));
        fs::create_dir_all(&dir).unwrap();
        let mut s = restricted_pair(&dir);
        let sum = summary(&mut s).unwrap();
        assert_eq!((sum.ready, sum.restricted), (0, 1));
        assert!(prepare(&mut s, false).unwrap().0.is_empty(), "left out by default");
        let (jobs, sum) = prepare(&mut s, true).unwrap();
        assert_eq!(jobs.len(), 1, "the person's choice");
        let done = write(&jobs, &sum, &dir, "2026-11-10").unwrap();
        let folder = PathBuf::from(done.folder);
        let readme = fs::read_to_string(folder.join("README.md")).unwrap();
        assert!(readme.contains("chose to include 0 clips a consent record in the project says no to and 1 with a restricted"), "{readme}");
        assert!(fs::read_to_string(folder.join("restricted")).unwrap().starts_with("seg_"));
        drop(s);
        fs::remove_dir_all(&dir).ok();
    }
}
