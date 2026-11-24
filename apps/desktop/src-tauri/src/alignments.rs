//! A forced aligner's results brought back: the Montreal Forced Aligner
//! writes a TextGrid per clip of the speech dataset (`speech_dataset.rs`),
//! named by the clip's id (`<speaker>-<segment id>`), with a words tier and
//! a phones tier timed from the clip's start. Each word and phone comes back
//! as a timed segment of the clip's text, on a track of its own (`words
//! (aligned)`, `phones (aligned)`, role `alignment`, so the corpus tools do
//! not count them twice), placed at the segment's start. Durations, Measure
//! and the vowel chart then work on single sounds.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::session::{self, Session};

/// Labels aligners write for silence, noise or a word they could not place.
const SILENT: &[&str] = &["", "sil", "sp", "spn", "<eps>", "<unk>", "sil0", "noise", "#"];

/// The role of the aligned tracks.
pub const ROLE: &str = "alignment";

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aligned {
    /// TextGrids read, and how many named a segment of this project.
    pub files: usize,
    pub clips: usize,
    pub words: usize,
    pub phones: usize,
    /// Files whose name names no live segment here (another project's, renamed).
    pub unmatched: Vec<String>,
    /// The first text brought into, to open.
    pub text: Option<String>,
}

fn grids(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() && depth < 3 {
            grids(&p, depth + 1, out);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("textgrid")) {
            out.push(p);
        }
    }
}

/// The segment a clip's file names: the part after the speaker and its `-`.
pub fn segment_of(stem: &str) -> Option<&str> {
    stem.split_once('-').map(|(_, seg)| seg).filter(|s| !s.is_empty())
}

/// Which kind of tier an aligner's tier is, by its name (`words`, `phones`,
/// or MFA's `<speaker> - words`).
fn kind_of(name: &str) -> Option<&'static str> {
    let n = name.trim().to_lowercase();
    if n.ends_with("words") || n == "word" {
        Some("words")
    } else if n.ends_with("phones") || n.ends_with("phonemes") || n == "phone" {
        Some("phones")
    } else {
        None
    }
}

fn track(p: &mut wugs_core::Project, made: &mut BTreeMap<&'static str, String>, kind: &'static str) -> Result<String, String> {
    if let Some(t) = made.get(kind) {
        return Ok(t.clone());
    }
    let name = format!("{kind} (aligned)");
    for t in p.proj.entities_of_kind("tier")? {
        if !p.proj.is_tombstoned(&t)? && p.proj.field_str(&t, "name")?.as_deref() == Some(name.as_str()) {
            made.insert(kind, t.clone());
            return Ok(t);
        }
    }
    let (t, _) = p.add_tier(&name, "time-aligned", Some(ROLE), None)?;
    made.insert(kind, t.clone());
    Ok(t)
}

/// Bring every TextGrid under `folder` (to three levels) back; a second time adds nothing.
pub fn import(s: &mut Session, folder: &Path) -> Result<Aligned, String> {
    let mut files = Vec::new();
    grids(folder, 0, &mut files);
    files.sort();
    if files.is_empty() {
        return Err("No TextGrid files were found there. Choose the folder the aligner wrote its results into.".into());
    }
    let p = session::project(s)?;
    let mut out = Aligned { files: files.len(), ..Aligned::default() };
    let mut made: BTreeMap<&'static str, String> = BTreeMap::new();
    for f in files {
        let stem = f.file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
        let seg = segment_of(&stem).map(str::to_string);
        let live = match &seg {
            Some(seg) => p.proj.entity_kind(seg)?.as_deref() == Some("segment") && !p.proj.is_tombstoned(seg)?,
            None => false,
        };
        let (Some(seg), true) = (seg, live) else {
            out.unmatched.push(stem);
            continue;
        };
        let (Some(text), Some(start)) = (p.proj.field_str(&seg, "text")?, p.proj.field_num(&seg, "start")?) else {
            out.unmatched.push(stem);
            continue;
        };
        let end = p.proj.field_num(&seg, "end")?.unwrap_or(f64::MAX);
        let input = wugs_legacy::decode(&std::fs::read(&f).map_err(|e| format!("{}: {e}", f.display()))?).map_err(|e| format!("{stem}: {e}"))?;
        let tiers = wugs_legacy::textgrid::tier_items(&input).map_err(|e| format!("{stem}: {e}"))?;
        let mut any = false;
        for (name, items) in tiers {
            let Some(kind) = kind_of(&name) else { continue };
            let rows: Vec<(f64, f64, String)> = items
                .into_iter()
                .filter(|(a, b, l)| b > a && !SILENT.contains(&l.trim().to_lowercase().as_str()))
                .map(|(a, b, l)| (start + a, (start + b).min(end), l))
                .filter(|(a, b, _)| b > a)
                .collect();
            if rows.is_empty() {
                continue;
            }
            let tier = track(p, &mut made, kind)?;
            let index = session::next_pos_index(p, &text)?;
            let (ids, _) = p.add_labelled_segments(&text, &tier, index, &rows, "import-alignment")?;
            if kind == "words" {
                out.words += ids.len();
            } else {
                out.phones += ids.len();
            }
            any = true;
        }
        if any {
            out.clips += 1;
            out.text.get_or_insert(text);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(words: &[(f64, f64, &str)], phones: &[(f64, f64, &str)]) -> String {
        let tier = |name: &str, items: &[(f64, f64, &str)]| {
            let mut t = format!("    item [{}]:\n        class = \"IntervalTier\"\n        name = \"{name}\"\n        xmin = 0\n        xmax = 2\n        intervals: size = {}\n", if name.ends_with("words") { 1 } else { 2 }, items.len());
            for (i, (a, b, l)) in items.iter().enumerate() {
                t.push_str(&format!("        intervals [{}]:\n            xmin = {a}\n            xmax = {b}\n            text = \"{l}\"\n", i + 1));
            }
            t
        };
        format!("File type = \"ooTextFile\"\nObject class = \"TextGrid\"\n\nxmin = 0\nxmax = 2\ntiers? <exists>\nsize = 2\nitem []:\n{}{}", tier("CHI - words", words), tier("CHI - phones", phones))
    }

    #[test]
    fn an_aligners_words_and_phones_come_back_on_their_own_tracks() {
        assert_eq!(segment_of("CHI-seg_01ABC"), Some("seg_01ABC"));
        assert_eq!((kind_of("utterance - words"), kind_of("phones"), kind_of("tones")), (Some("words"), Some("phones"), None));
        let dir = std::env::temp_dir().join(format!("wugs-align-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let p = session::project(&mut s).unwrap();
        let tier = session::utterance_tier(p).unwrap();
        let text = p.create_text("Story").unwrap();
        let (seg, _) = p.add_segment(&text, &tier, 0, Some(10_000.0), Some(11_500.0), "kani ŋai").unwrap();
        let out = dir.join("aligned").join("CHI");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join(format!("CHI-{seg}.TextGrid")), grid(&[(0.0, 0.1, ""), (0.1, 0.6, "kani"), (0.6, 0.7, "sil"), (0.7, 1.2, "ŋai")], &[(0.1, 0.2, "k"), (0.2, 0.35, "a"), (0.35, 0.45, "n"), (0.45, 0.6, "i"), (0.6, 0.7, "sil")])).unwrap();
        std::fs::write(out.join("CHI-gone.TextGrid"), grid(&[], &[])).unwrap();
        let r = import(&mut s, &dir.join("aligned")).unwrap();
        assert_eq!((r.files, r.clips, r.words, r.phones), (2, 1, 2, 4), "{r:?}");
        assert_eq!(r.unmatched, vec!["CHI-gone".to_string()]);
        let detail = session::get_text(&mut s, &text).unwrap();
        let words = detail.tiers.iter().find(|t| t.name == "words (aligned)").unwrap().id.clone();
        let kani = detail.segments.iter().find(|x| x.tier == words && x.content == "kani").unwrap();
        assert_eq!((kani.start_ms, kani.end_ms), (Some(10_100.0), Some(10_600.0)), "placed at the segment's start");
        let again = import(&mut s, &dir.join("aligned")).unwrap();
        assert_eq!((again.words, again.phones), (0, 0), "a second import adds nothing");
        let p = session::project(&mut s).unwrap();
        let (_, transcription) = wugs_cldf::corpus_tiers(p).unwrap();
        assert!(!transcription.contains(&words), "aligned words are not counted as transcription");
        std::fs::remove_dir_all(&dir).ok();
    }
}
