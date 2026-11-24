//! A speech dataset of the project (roadmap 52 and 58): every timed segment
//! a person transcribed, as a Kaldi-style folder of 16 kHz clips with their
//! transcriptions, speakers (tracks) and texts, the shape speech toolkits
//! (Kaldi, ESPnet, k2/icefall, Hugging Face's audio folders) read, for
//! training and testing models on the language documented here.
//!
//! As with the corrections export (`corrections.rs`), consent records and
//! access levels set the defaults and never decide: clips a record says no
//! to (for AI use) or with a restricted level start left out, the person
//! may include them, and the datasheet says what the records say for every
//! clip. The session thread gathers (`prepare`); cutting runs off it (`write`).

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use wugs_core::assist::{Consent, Transcribed, TranscribedClip};

use crate::corrections::{write_wav, ARRIVING, GUEST};
use crate::session::{self, Session};

#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Clips nothing in the project's records speaks against.
    pub ready: usize,
    /// Clips a consent record says no to (for AI use).
    pub recorded_no: usize,
    /// Clips with a restricted or community access level (and no recorded no).
    pub restricted: usize,
    pub untimed: usize,
    pub no_recording: usize,
    pub unchecked: usize,
    /// Why this device may not know every consent record, or None.
    pub partial: Option<String>,
}

pub struct Job {
    clip: TranscribedClip,
    wav: PathBuf,
    speaker: String,
    title: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    pub folder: String,
    pub written: usize,
    pub failed: usize,
    pub problems: Vec<String>,
}

fn flagged(c: &TranscribedClip) -> bool {
    c.consent == Consent::No || c.restricted
}

fn gather(s: &mut Session) -> Result<Transcribed, String> {
    let p = session::project(s)?;
    let (_, transcription) = wugs_cldf::corpus_tiers(p)?;
    let tiers: HashSet<String> = transcription.into_iter().collect();
    p.transcribed(&tiers)
}

fn tally(t: &Transcribed, guest: bool) -> Summary {
    let partial = if guest { Some(GUEST.to_string()) } else if t.records_incomplete { Some(ARRIVING.to_string()) } else { None };
    let mut sum = Summary { untimed: t.untimed, no_recording: t.no_recording, unchecked: t.unchecked, partial, ..Summary::default() };
    for c in &t.clips {
        if c.consent == Consent::No {
            sum.recorded_no += 1;
        } else if c.restricted {
            sum.restricted += 1;
        } else {
            sum.ready += 1;
        }
    }
    sum
}

pub fn summary(s: &mut Session, guest: bool) -> Result<Summary, String> {
    Ok(tally(&gather(s)?, guest))
}

/// A speaker id for Kaldi's tables: the track's name, letters, digits and `_` only.
fn speaker_id(name: &str, tier: &str) -> String {
    let clean: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let clean = clean.trim_matches('_');
    if clean.is_empty() {
        tier.chars().filter(char::is_ascii_alphanumeric).collect()
    } else {
        clean.to_string()
    }
}

pub fn prepare(s: &mut Session, guest: bool, include_flagged: bool) -> Result<(Vec<Job>, Summary), String> {
    let t = gather(s)?;
    let sum = tally(&t, guest);
    let p = session::project(s)?;
    let mut jobs = Vec::new();
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut titles: BTreeMap<String, String> = BTreeMap::new();
    let mut wavs: BTreeMap<String, Option<PathBuf>> = BTreeMap::new();
    for clip in t.clips {
        if flagged(&clip) && !include_flagged {
            continue;
        }
        if !wavs.contains_key(&clip.source) {
            let wav = session::audio_blob(p, &clip.source).ok().and_then(|b| wugs_blob::parse_ref(&b).map(|h| p.root.join("blobs").join(h))).filter(|w| w.is_file());
            wavs.insert(clip.source.clone(), wav);
        }
        let Some(wav) = wavs[&clip.source].clone() else { continue };
        if !names.contains_key(&clip.tier) {
            let name = p.proj.field_str(&clip.tier, "name")?.unwrap_or_default();
            names.insert(clip.tier.clone(), speaker_id(&name, &clip.tier));
        }
        if !titles.contains_key(&clip.text) {
            titles.insert(clip.text.clone(), p.proj.field_str(&clip.text, "title")?.unwrap_or_default());
        }
        jobs.push(Job { speaker: names[&clip.tier].clone(), title: titles[&clip.text].clone(), clip, wav });
    }
    Ok((jobs, sum))
}

/// Write `wugs-speech-<today>/`: `wav/<utt>.wav` (16 kHz mono PCM16),
/// `wav.scp`, `text`, `utt2spk`, `spk2utt`, `utt2dur`, `utt2text` (the text's
/// title), `utt2consent`, `restricted`, `metadata.csv` (file name and
/// transcription, as Hugging Face's audio folders read it) and a README
/// datasheet. Utterance ids are `<speaker>-<segment id>`, so Kaldi's sorted
/// tables stay grouped by speaker.
pub fn write(jobs: &[Job], sum: &Summary, parent: &Path, today: &str) -> Result<Exported, String> {
    if jobs.is_empty() {
        return Err("There are no transcribed, timed segments with a recording here to export.".into());
    }
    let mut folder = parent.join(format!("wugs-speech-{today}"));
    let mut n = 2;
    while folder.exists() {
        folder = parent.join(format!("wugs-speech-{today}-{n}"));
        n += 1;
    }
    fs::create_dir_all(folder.join("wav")).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut tables: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut spk2utt: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut metadata = vec!["file_name,transcription,speaker,text".to_string()];
    let (mut written, mut failed, mut problems) = (0, 0, Vec::new());
    let (mut yes, mut unrecorded, mut no, mut restricted, mut seconds) = (0, 0, 0, 0, 0.0);
    let csv = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    for job in jobs {
        let c = &job.clip;
        // A synced id is untrusted: it names a file, so only letters, digits and `_`.
        if c.segment.is_empty() || !c.segment.chars().all(|x| x.is_ascii_alphanumeric() || x == '_') {
            failed += 1;
            continue;
        }
        let utt = format!("{}-{}", job.speaker, c.segment);
        let file = format!("wav/{utt}.wav");
        let cut = wugs_asr::read_span(&job.wav, c.start_ms, c.end_ms).map_err(|e| e.to_string()).and_then(|audio| write_wav(&folder.join(&file), &audio));
        if let Err(e) = cut {
            failed += 1;
            if problems.len() < 3 {
                problems.push(format!("«{}» at {:.1} s: {e}", job.title, c.start_ms / 1000.0));
            }
            continue;
        }
        written += 1;
        let one_line = c.transcription.split_whitespace().collect::<Vec<_>>().join(" ");
        let dur = (c.end_ms - c.start_ms) / 1000.0;
        seconds += dur;
        let consent = if c.consent == Consent::Unrecorded && sum.partial.is_some() { "unknown" } else { c.consent.as_str() };
        for (table, value) in [("wav.scp", file.clone()), ("text", one_line.clone()), ("utt2spk", job.speaker.clone()), ("utt2dur", format!("{dur:.3}")), ("utt2text", job.title.split_whitespace().collect::<Vec<_>>().join(" ")), ("utt2consent", consent.to_string())] {
            tables.entry(table).or_default().push(format!("{utt} {value}"));
        }
        spk2utt.entry(job.speaker.clone()).or_default().push(utt.clone());
        if c.restricted {
            restricted += 1;
            tables.entry("restricted").or_default().push(utt.clone());
        }
        match c.consent {
            Consent::Yes => yes += 1,
            Consent::Unrecorded => unrecorded += 1,
            Consent::No => no += 1,
        }
        metadata.push(format!("{},{},{},{}", csv(&file), csv(&one_line), csv(&job.speaker), csv(&job.title)));
    }
    if written == 0 {
        fs::remove_dir_all(&folder).ok();
        return Err(format!("No clip could be cut. {}", problems.join(" ")));
    }
    let included_flagged = jobs.iter().any(|j| flagged(&j.clip));
    let left_out = if included_flagged || sum.recorded_no + sum.restricted == 0 {
        String::new()
    } else {
        format!(" Left out (the default, which the exporter did not change): {} clips a consent record says no to, and {} more with a restricted or community access level.", sum.recorded_no, sum.restricted)
    };
    let partial = sum.partial.as_ref().map(|w| format!(" Note: {w} Such clips read `unknown` in `utt2consent`.")).unwrap_or_default();
    let readme = format!(
        "# Speech dataset (wugs)\n\n\
Exported {today}: {written} clips, {:.1} minutes, from {} speakers (tracks), each a segment a person transcribed in wugs.\n\n\
**These are recordings of real people. Treat this folder as you would the recordings themselves:** share it only as far as the speakers agreed.\n\n\
## What the project's records say\n\n\
Of these clips, {yes} are covered by a consent record that allows AI use, {unrecorded} by none that speaks about it, and {no} by one that does not allow it; {restricted} have a restricted or community access level.{partial}{left_out} Not in this export: {} transcribed segments with no times, {} whose text has no recording linked, {} still an unchecked machine suggestion.{}\n\n\
## Files\n\n\
Kaldi-style tables, one line per clip, keyed by `<speaker>-<segment id>` and sorted:\n\n\
- `wav/<id>.wav`: the clip, 16 kHz mono 16-bit PCM\n\
- `wav.scp`, `text` (the transcription, on one line), `utt2spk`, `spk2utt`, `utt2dur` (seconds)\n\
- `utt2text`: the title of the text it comes from\n\
- `utt2consent`: what consent records say about AI use (`recorded-yes`, `not-recorded`, `recorded-no`, or `unknown`)\n\
- `restricted`: clips whose text or recording has a restricted or community access level\n\
- `metadata.csv`: file name, transcription, speaker and text, as Hugging Face's `audiofolder` reads it\n\n\
A speaker is a transcription track (often one per person in ELAN files); rename tracks in wugs to name speakers. The transcriptions follow the project's own conventions (phonetic, phonemic or orthographic). Split train and test sets by speaker or by text, never by clip, or a model is tested on what it heard.\n",
        seconds / 60.0,
        spk2utt.len(),
        sum.untimed,
        sum.no_recording,
        sum.unchecked,
        if failed > 0 { format!(" {failed} chosen clips could not be cut. {}", problems.join(" ")) } else { String::new() },
    );
    let finish = || -> Result<(), String> {
        for (name, mut lines) in tables.clone() {
            lines.sort();
            fs::write(folder.join(name), lines.join("\n") + "\n").map_err(|e| format!("{name}: {e}"))?;
        }
        let spk: Vec<String> = spk2utt.iter().map(|(s, u)| {
            let mut u = u.clone();
            u.sort();
            format!("{s} {}", u.join(" "))
        }).collect();
        fs::write(folder.join("spk2utt"), spk.join("\n") + "\n").map_err(|e| format!("spk2utt: {e}"))?;
        fs::write(folder.join("metadata.csv"), metadata.join("\n") + "\n").map_err(|e| format!("metadata.csv: {e}"))?;
        fs::write(folder.join("README.md"), &readme).map_err(|e| format!("README.md: {e}"))
    };
    if let Err(e) = finish() {
        fs::remove_dir_all(&folder).ok();
        return Err(e);
    }
    Ok(Exported { folder: folder.display().to_string(), written, failed, problems })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaker_ids_are_safe() {
        assert_eq!(speaker_id("Ana (speaker 1)", "t1"), "Ana__speaker_1");
        assert_eq!(speaker_id("ŋ", "tier_01ABC"), "tier01ABC");
    }

    #[test]
    fn the_tutorial_exports_as_a_speech_dataset() {
        let dir = std::env::temp_dir().join(format!("wugs-speech-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "tester").unwrap();
        let sum = summary(&mut s, false).unwrap();
        assert_eq!(sum.ready, 2, "the two transcribed utterances, not the empty one or translations: {sum:?}");
        assert_eq!((sum.recorded_no, sum.restricted, sum.partial.is_none()), (0, 0, true));
        let (jobs, sum) = prepare(&mut s, false, false).unwrap();
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        let r = write(&jobs, &sum, &out, "2026-11-24").unwrap();
        assert_eq!((r.written, r.failed), (2, 0), "{r:?}");
        let folder = Path::new(&r.folder);
        assert!(r.folder.ends_with("wugs-speech-2026-11-24"));
        let text = fs::read_to_string(folder.join("text")).unwrap();
        assert!(text.contains(" wʌn tuː θɹiː\n") && text.contains(" fɔɹ faɪv\n"), "{text}");
        let scp = fs::read_to_string(folder.join("wav.scp")).unwrap();
        let first = scp.lines().next().unwrap().split(' ').nth(1).unwrap();
        assert_eq!(&fs::read(folder.join(first)).unwrap()[..4], b"RIFF");
        let spk = fs::read_to_string(folder.join("spk2utt")).unwrap();
        assert_eq!(spk.lines().count(), 1, "one track: {spk}");
        assert!(fs::read_to_string(folder.join("metadata.csv")).unwrap().starts_with("file_name,transcription,speaker,text\n\"wav/"));
        assert!(fs::read_to_string(folder.join("utt2consent")).unwrap().lines().all(|l| l.ends_with(" not-recorded")));
        drop(s);
        fs::remove_dir_all(&dir).ok();
    }
}
