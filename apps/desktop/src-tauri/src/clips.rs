//! Save a track's segments as sound files: one WAV per timed segment, cut
//! from the recording at its own quality, with a `clips.csv` index (what
//! Praat's "Extract all intervals" gives, and what stimulus lists, flash
//! cards and playback experiments start from).
//!
//! The folder `<text> <track> clips` (then ` (2)` …) is written whole beside
//! a `.partial` scratch folder and renamed, so a failure leaves nothing
//! half-written. Files are numbered in time order and named after their
//! transcription, so they sort and read in a file browser.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::interchange::{cut_clip, file_stem, translations_of};
use crate::session::{self, Session};

pub struct Row {
    pub start_ms: f64,
    pub end_ms: f64,
    pub transcription: String,
    pub translation: String,
}

/// Everything the cutting needs, gathered on the session thread.
pub struct Prepared {
    wav: PathBuf,
    text: String,
    track: String,
    rows: Vec<Row>,
    untimed: usize,
}

/// The timed segments of `tier_id` in `text_id`, in time order, and the
/// WAV of `source` (the recording the Timeline has loaded for the text).
pub fn prepare(s: &mut Session, text_id: &str, tier_id: &str, source: &str) -> Result<Prepared, String> {
    let detail = session::get_text(s, text_id)?;
    let track = detail.tiers.iter().find(|t| t.id == tier_id).map(|t| t.name.clone()).ok_or("That track is not in this text.")?;
    let p = session::project(s)?;
    let blob = session::audio_blob(p, source)?;
    let hex = wugs_blob::parse_ref(&blob).ok_or_else(|| format!("malformed blob ref {blob:?}"))?;
    let wav = p.root.join("blobs").join(hex);
    if !wav.is_file() {
        return Err("The recording's audio file is not on this computer.".into());
    }
    let mut rows = Vec::new();
    let mut untimed = 0;
    for seg in detail.segments.iter().filter(|x| x.tier == tier_id) {
        match (seg.start_ms, seg.end_ms) {
            (Some(a), Some(b)) if b > a => {
                let translation = translations_of(p, &seg.id)?.into_iter().map(|t| t.1).next().unwrap_or_default();
                rows.push(Row { start_ms: a, end_ms: b, transcription: seg.content.clone(), translation });
            }
            _ => untimed += 1,
        }
    }
    if rows.is_empty() {
        return Err(format!("«{track}» has no timed segments to save."));
    }
    rows.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms).then(a.end_ms.total_cmp(&b.end_ms)));
    Ok(Prepared { wav, text: detail.title, track, rows, untimed })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipsReport {
    pub path: String,
    pub clips: usize,
    pub untimed: usize,
}

/// A clip's file name: its number, then its transcription (at most 40
/// characters, made safe for every file system), so files sort in time order.
pub fn clip_name(n: usize, width: usize, transcription: &str) -> String {
    let short: String = transcription.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(40).collect();
    let stem = file_stem(short.trim(), "");
    if stem.is_empty() {
        format!("{n:0width$}.wav")
    } else {
        format!("{n:0width$} {stem}.wav")
    }
}

/// Cut every clip into a new folder inside `folder` (off the session thread).
pub fn write(prepared: Prepared, folder: &Path) -> Result<ClipsReport, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    let base = file_stem(&format!("{} {} clips", prepared.text, prepared.track), "clips");
    let mut out = folder.join(&base);
    let mut k = 2;
    while out.symlink_metadata().is_ok() {
        out = folder.join(format!("{base} ({k})"));
        k += 1;
    }
    let scratch = folder.join(format!(".{}.partial", out.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let width = prepared.rows.len().to_string().len().max(3);
    let written = (|| {
        fs::create_dir(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
        w.write_record(["file", "start", "end", "duration", "transcription", "translation"]).map_err(|e| e.to_string())?;
        for (i, r) in prepared.rows.iter().enumerate() {
            let name = clip_name(i + 1, width, &r.transcription);
            cut_clip(&prepared.wav, r.start_ms, r.end_ms, &scratch.join(&name)).map_err(|e| format!("{name}: {e}"))?;
            let secs = |ms: f64| format!("{:.3}", ms / 1000.0);
            w.write_record([name.as_str(), &secs(r.start_ms), &secs(r.end_ms), &secs(r.end_ms - r.start_ms), &r.transcription, &r.translation]).map_err(|e| e.to_string())?;
        }
        let mut index = "\u{feff}".as_bytes().to_vec();
        index.extend(w.into_inner().map_err(|e| e.to_string())?);
        fs::write(scratch.join("clips.csv"), index).map_err(|e| format!("clips.csv: {e}"))?;
        fs::rename(&scratch, &out).map_err(|e| format!("{}: {e}", out.display()))
    })();
    if let Err(e) = written {
        fs::remove_dir_all(&scratch).ok();
        return Err(e);
    }
    Ok(ClipsReport { path: out.display().to_string(), clips: prepared.rows.len(), untimed: prepared.untimed })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_sort_and_stay_safe() {
        assert_eq!(clip_name(7, 3, "wʌn  tuː θɹiː"), "007 wʌn tuː θɹiː.wav");
        assert_eq!(clip_name(12, 3, ""), "012.wav");
        assert_eq!(clip_name(1, 4, "a/b: c?"), "0001 a_b_ c.wav");
        assert_eq!(clip_name(1, 3, &"x".repeat(60)).chars().count(), "001 ".len() + 40 + ".wav".len());
    }

    #[test]
    fn the_tutorial_track_comes_out_as_clips() {
        let dir = std::env::temp_dir().join(format!("wugs-clips-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "tester").unwrap();
        let text = session::list_texts(&mut s).unwrap().into_iter().next().unwrap().id;
        let detail = session::get_text(&mut s, &text).unwrap();
        let track = detail.tiers.iter().find(|t| t.utterance).unwrap().id.clone();
        let source = session::project(&mut s).unwrap().text_recording(&text).unwrap().expect("the tutorial comes linked to its recording").0;
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        let r = write(prepare(&mut s, &text, &track, &source).unwrap(), &out).unwrap();
        assert!(r.clips >= 3, "{r:?}");
        let folder = Path::new(&r.path);
        let index = fs::read_to_string(folder.join("clips.csv")).unwrap();
        let mut lines = index.lines();
        assert_eq!(lines.next().unwrap(), "\u{feff}file,start,end,duration,transcription,translation");
        let first = lines.next().unwrap();
        assert!(first.starts_with("001 wʌn tuː θɹiː.wav,") && first.ends_with(",wʌn tuː θɹiː,\"One, two, three.\""), "{first}");
        let wav = fs::read(folder.join("001 wʌn tuː θɹiː.wav")).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        // A second save goes beside the first, never over it.
        let again = write(prepare(&mut s, &text, &track, &source).unwrap(), &out).unwrap();
        assert!(again.path.ends_with("clips (2)"), "{}", again.path);
        assert!(!fs::read_dir(&out).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".partial")));
        drop(s);
        fs::remove_dir_all(dir).ok();
    }
}
