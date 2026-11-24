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
    /// The recording it is cut from.
    pub wav: PathBuf,
    pub start_ms: f64,
    pub end_ms: f64,
    pub transcription: String,
    pub translation: String,
    /// For concordance lines: the text's title and what was searched for.
    pub line: Option<(String, String)>,
}

/// Everything the cutting needs, gathered on the session thread.
pub struct Prepared {
    /// The folder's name before ` (2)` and the like.
    base: String,
    rows: Vec<Row>,
    untimed: usize,
    /// Concordance lines whose text has no recording on this computer.
    missing: usize,
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
                rows.push(Row { wav: wav.clone(), start_ms: a, end_ms: b, transcription: seg.content.clone(), translation, line: None });
            }
            _ => untimed += 1,
        }
    }
    if rows.is_empty() {
        return Err(format!("«{track}» has no timed segments to save."));
    }
    rows.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms).then(a.end_ms.total_cmp(&b.end_ms)));
    Ok(Prepared { base: format!("{} {track} clips", detail.title), rows, untimed, missing: 0 })
}

/// A concordance line to save: its segment and what was searched for.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HitRef {
    pub text_id: String,
    pub segment_id: String,
    pub keyword: String,
}

/// The sentences behind concordance lines, each once in the order given,
/// cut from each text's own recording (`Project::text_recording`); lines
/// with no times, or whose text has no recording here, are counted.
pub fn prepare_hits(s: &mut Session, query: &str, hits: &[HitRef]) -> Result<Prepared, String> {
    let p = session::project(s)?;
    let mut rows = Vec::new();
    let (mut untimed, mut missing) = (0, 0);
    let mut seen = std::collections::HashSet::new();
    let mut titles: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut wavs: std::collections::HashMap<String, Option<PathBuf>> = std::collections::HashMap::new();
    for h in hits {
        if !seen.insert(h.segment_id.clone()) {
            continue;
        }
        let (Some(a), Some(b)) = (p.proj.field_num(&h.segment_id, "start")?, p.proj.field_num(&h.segment_id, "end")?) else {
            untimed += 1;
            continue;
        };
        if b <= a {
            untimed += 1;
            continue;
        }
        if !wavs.contains_key(&h.text_id) {
            let wav = match p.text_recording(&h.text_id)? {
                Some((_, blob)) => wugs_blob::parse_ref(&blob).map(|hex| p.root.join("blobs").join(hex)).filter(|w| w.is_file()),
                None => None,
            };
            wavs.insert(h.text_id.clone(), wav);
        }
        let Some(wav) = wavs[&h.text_id].clone() else {
            missing += 1;
            continue;
        };
        if !titles.contains_key(&h.text_id) {
            titles.insert(h.text_id.clone(), p.proj.field_str(&h.text_id, "title")?.unwrap_or_default());
        }
        let transcription = p.proj.field_str(&h.segment_id, "content")?.unwrap_or_default();
        let translation = translations_of(p, &h.segment_id)?.into_iter().map(|t| t.1).next().unwrap_or_default();
        rows.push(Row { wav, start_ms: a, end_ms: b, transcription, translation, line: Some((titles[&h.text_id].clone(), h.keyword.clone())) });
    }
    if rows.is_empty() {
        return Err(if missing > 0 { "None of these lines' texts has its recording on this computer: pick each text's recording in the Timeline first.".into() } else { "None of these lines is timed, so there is nothing to cut.".into() });
    }
    let short: String = query.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(40).collect();
    Ok(Prepared { base: format!("{} clips", if short.is_empty() { "concordance" } else { short.as_str() }), rows, untimed, missing })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipsReport {
    pub path: String,
    pub clips: usize,
    pub untimed: usize,
    /// Concordance lines whose text has no recording here.
    pub missing: usize,
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
    let base = file_stem(&prepared.base, "clips");
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
        let lines = prepared.rows.iter().any(|r| r.line.is_some());
        let mut head = vec!["file", "start", "end", "duration", "transcription", "translation"];
        if lines {
            head.extend(["text", "keyword"]);
        }
        w.write_record(&head).map_err(|e| e.to_string())?;
        for (i, r) in prepared.rows.iter().enumerate() {
            let name = clip_name(i + 1, width, &r.transcription);
            cut_clip(&r.wav, r.start_ms, r.end_ms, &scratch.join(&name)).map_err(|e| format!("{name}: {e}"))?;
            let secs = |ms: f64| format!("{:.3}", ms / 1000.0);
            let mut record = vec![name.clone(), secs(r.start_ms), secs(r.end_ms), secs(r.end_ms - r.start_ms), r.transcription.clone(), r.translation.clone()];
            if let Some((text, keyword)) = &r.line {
                record.extend([text.clone(), keyword.clone()]);
            }
            w.write_record(&record).map_err(|e| e.to_string())?;
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
    Ok(ClipsReport { path: out.display().to_string(), clips: prepared.rows.len(), untimed: prepared.untimed, missing: prepared.missing })
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
        // Concordance lines: each sentence once, from its text's recording, with the text and keyword.
        let first_seg = detail.segments.iter().filter(|x| x.tier == track && x.start_ms.is_some()).min_by(|a, b| a.start_ms.unwrap().total_cmp(&b.start_ms.unwrap())).unwrap().id.clone();
        let hit = |seg: &str| HitRef { text_id: text.clone(), segment_id: seg.to_string(), keyword: "tuː".into() };
        let r = write(prepare_hits(&mut s, "tuː", &[hit(&first_seg), hit(&first_seg)]).unwrap(), &out).unwrap();
        assert_eq!((r.clips, r.untimed, r.missing), (1, 0, 0), "{r:?}");
        assert!(r.path.ends_with("tuː clips"), "{}", r.path);
        let index = fs::read_to_string(Path::new(&r.path).join("clips.csv")).unwrap();
        assert!(index.lines().next().unwrap().ends_with(",text,keyword"), "{index}");
        assert!(index.lines().nth(1).unwrap().ends_with(",Counting to six (tutorial),tuː"), "{index}");
        drop(s);
        fs::remove_dir_all(dir).ok();
    }
}
