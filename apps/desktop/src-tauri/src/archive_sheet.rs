//! A catalogue sheet for an archive deposit: one row per text with what an
//! archive's item form asks (title, date, place, speakers, genre, language
//! and its codes, the recording and its length, the sharing level) and how
//! far the text has come (sentences transcribed and translated, words
//! glossed), then every other detail the texts carry. ELAR, PARADISEC and
//! AILLA take their item metadata as a spreadsheet, and lameta imports
//! sessions from one, so this is the table to fill their forms from.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::session::{self, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sheet {
    pub path: String,
    pub texts: usize,
    /// Texts with no recording linked, or whose recording is not here.
    pub without_recording: usize,
    /// Texts whose sharing level is not open.
    pub not_open: usize,
}

/// The details printed in their own columns, in this order, each with the
/// names a text's details may carry it under.
const NAMED: &[(&str, &[&str])] = &[
    ("date", &["date", "recorded", "recording_date"]),
    ("place", &["place", "location"]),
    ("speakers", &["speakers", "speaker", "participants"]),
    ("genre", &["genre"]),
    ("description", &["description", "summary", "notes"]),
    ("keywords", &["keywords"]),
];

pub fn export_sheet(s: &mut Session, folder: &Path) -> Result<Sheet, String> {
    crate::interchange::outside_project(s, folder)?;
    let progress = crate::progress::progress(s)?;
    let texts = session::list_texts(s)?;
    if texts.is_empty() {
        return Err("The project has no texts yet.".into());
    }
    let p = session::project(s)?;
    let lang = p.project_language()?;
    let used: BTreeSet<&str> = NAMED.iter().flat_map(|(_, keys)| keys.iter().copied()).collect();
    let mut metas = Vec::new();
    let mut extra: BTreeSet<String> = BTreeSet::new();
    for t in &texts {
        let m = p.text_metadata(&t.id)?;
        extra.extend(m.iter().map(|(k, _)| k.clone()).filter(|k| !used.contains(k.as_str())));
        metas.push(m);
    }
    let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
    let mut head: Vec<String> = vec!["title".into()];
    head.extend(NAMED.iter().map(|(n, _)| n.to_string()));
    head.extend(
        ["language", "iso639_3", "glottocode", "access", "recording", "recording_seconds", "sentences", "transcribed", "translated", "words", "glossed_words"]
            .iter()
            .map(|s| s.to_string()),
    );
    head.extend(extra.iter().cloned());
    w.write_record(&head).map_err(|e| e.to_string())?;
    let mut sheet = Sheet { path: String::new(), texts: texts.len(), without_recording: 0, not_open: 0 };
    for (t, meta) in texts.iter().zip(&metas) {
        // What the text does not say, its recording's details may (date, place…).
        let recorded = match p.text_recording(&t.id)? {
            Some((source, _)) => p.source_details(&source)?,
            None => Vec::new(),
        };
        let find = |m: &[(String, String)], keys: &[&str]| keys.iter().find_map(|k| m.iter().find(|(mk, _)| mk == k).map(|(_, v)| v.clone()));
        let get = |keys: &[&str]| find(meta, keys).or_else(|| find(&recorded, keys)).unwrap_or_default();
        let mut row = vec![t.title.clone()];
        row.extend(NAMED.iter().map(|(_, keys)| get(keys)));
        let access = p.text_access(&t.id)?;
        if access != "open" {
            sheet.not_open += 1;
        }
        let stats = progress.texts.iter().find(|x| x.id == t.id);
        let recording = p.text_recording(&t.id)?.map(|(source, _)| p.proj.field_str(&source, "label").ok().flatten().unwrap_or(source));
        let seconds = stats.and_then(|x| x.recording_ms).map(|ms| format!("{:.1}", ms / 1000.0));
        if recording.is_none() || seconds.is_none() {
            sheet.without_recording += 1;
        }
        row.extend([lang.name.clone(), lang.iso.clone(), lang.glottocode.clone(), access, recording.unwrap_or_default(), seconds.unwrap_or_default()]);
        let n = |f: fn(&crate::progress::TextProgress) -> usize| stats.map_or(0, f).to_string();
        row.extend([n(|x| x.sentences), n(|x| x.transcribed), n(|x| x.translated), n(|x| x.words), n(|x| x.glossed_words)]);
        row.extend(extra.iter().map(|k| meta.iter().find(|(mk, _)| mk == k).map(|(_, v)| v.clone()).unwrap_or_default()));
        w.write_record(&row).map_err(|e| e.to_string())?;
    }
    let mut bytes = "\u{feff}".as_bytes().to_vec();
    bytes.extend(w.into_inner().map_err(|e| e.to_string())?);
    let name = if lang.name.trim().is_empty() { "texts for the archive".to_string() } else { format!("{} texts for the archive", lang.name.trim()) };
    let path = crate::interchange::write_new(folder, &crate::interchange::file_stem(&name, "texts for the archive"), "csv", &bytes)?;
    sheet.path = path.display().to_string();
    Ok(sheet)
}

/// What an archive's item form asks of a text that it does not have yet.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextReadiness {
    pub text_id: String,
    pub title: String,
    /// Each of `date`, `place`, `speakers`, `genre`, `description` (from the
    /// text's details, else its recording's), `recording` (none linked),
    /// `consent` (no record covers it), `transcription` (sentences left to
    /// transcribe, or none at all) and `translation` (transcribed sentences
    /// with no translation) that applies, in that order.
    pub missing: Vec<String>,
    pub sentences: usize,
    pub transcribed: usize,
    pub translated: usize,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Readiness {
    pub total: usize,
    /// Texts with nothing missing.
    pub ready: usize,
    /// Texts with something missing, those missing most first.
    pub texts: Vec<TextReadiness>,
    /// What the project's language lacks: `name`, `glottocode`, `iso639_3`.
    pub language: Vec<String>,
}

/// The details an item form asks of every text, as the catalogue sheet reads them.
const ASKED: &[&str] = &["date", "place", "speakers", "genre", "description"];

/// Which texts are ready for an archive deposit, and what each still lacks:
/// the catalogue sheet's questions asked before it is written. Information
/// only (records and details never block anything).
pub fn readiness(s: &mut Session) -> Result<Readiness, String> {
    let progress = crate::progress::progress(s)?;
    let consent = crate::people::consent_gaps(s)?;
    let texts = session::list_texts(s)?;
    let p = session::project(s)?;
    let lang = p.project_language()?;
    let mut out = Readiness { total: texts.len(), ..Readiness::default() };
    for (field, value) in [("name", &lang.name), ("glottocode", &lang.glottocode), ("iso639_3", &lang.iso)] {
        if value.trim().is_empty() {
            out.language.push(field.into());
        }
    }
    for t in &texts {
        let meta = p.text_metadata(&t.id)?;
        let recording = p.text_recording(&t.id)?;
        let recorded = match &recording {
            Some((source, _)) => p.source_details(source)?,
            None => Vec::new(),
        };
        let has = |keys: &[&str]| keys.iter().any(|k| meta.iter().chain(&recorded).any(|(mk, v)| mk == k && !v.trim().is_empty()));
        let mut missing: Vec<String> = Vec::new();
        for name in ASKED {
            let keys = NAMED.iter().find(|(n, _)| n == name).map(|(_, k)| *k).unwrap_or(&[]);
            if !has(keys) {
                missing.push((*name).into());
            }
        }
        if recording.is_none() {
            missing.push("recording".into());
        }
        if consent.texts.iter().any(|c| c.text_id == t.id && c.records == 0) {
            missing.push("consent".into());
        }
        let stats = progress.texts.iter().find(|x| x.id == t.id);
        let (sentences, transcribed, translated) = stats.map_or((0, 0, 0), |x| (x.sentences, x.transcribed, x.translated));
        if sentences == 0 || transcribed < sentences {
            missing.push("transcription".into());
        }
        if translated < transcribed {
            missing.push("translation".into());
        }
        if missing.is_empty() {
            out.ready += 1;
        } else {
            out.texts.push(TextReadiness { text_id: t.id.clone(), title: t.title.clone(), missing, sentences, transcribed, translated });
        }
    }
    out.texts.sort_by_cached_key(|t| (std::cmp::Reverse(t.missing.len()), wugs_model::fold_key(&t.title), t.text_id.clone()));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tutorial_becomes_one_catalogue_row() {
        let dir = std::env::temp_dir().join(format!("wugs-archive-sheet-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        session::project(&mut s).unwrap().set_project_language("Tutorial English", "stan1293", "eng").unwrap();
        let text = session::list_texts(&mut s).unwrap()[0].id.clone();
        let p = session::project(&mut s).unwrap();
        p.set_text_metadata(&text, "Speakers", "J. C.").unwrap();
        p.set_text_metadata(&text, "Place", "Home").unwrap();
        p.set_text_metadata(&text, "Session", "S1").unwrap();
        // The recording's date fills the text's empty date; the text's own place wins.
        let (source, _) = p.text_recording(&text).unwrap().unwrap();
        p.set_source_detail(&source, "date", "2026-11-03").unwrap();
        p.set_source_detail(&source, "place", "Studio").unwrap();
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let r = export_sheet(&mut s, &out).unwrap();
        assert_eq!((r.texts, r.without_recording, r.not_open), (1, 0, 0), "{r:?}");
        assert!(r.path.ends_with("Tutorial English texts for the archive.csv"), "{}", r.path);
        let csv = std::fs::read_to_string(&r.path).unwrap();
        let mut lines = csv.trim_start_matches('\u{feff}').lines();
        let head: Vec<&str> = lines.next().unwrap().split(',').collect();
        assert_eq!(&head[..7], ["title", "date", "place", "speakers", "genre", "description", "keywords"]);
        assert_eq!(head.last(), Some(&"session"), "other details follow");
        let row = lines.next().unwrap();
        assert!(row.starts_with("Counting to six (tutorial),2026-11-03,Home,J. C.,,,,Tutorial English,eng,stan1293,open,"), "{row}");
        assert!(row.ends_with(",S1"), "{row}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn readiness_says_what_each_text_lacks() {
        let dir = std::env::temp_dir().join(format!("wugs-archive-ready-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        let text = session::list_texts(&mut s).unwrap()[0].id.clone();
        let r = readiness(&mut s).unwrap();
        assert_eq!((r.total, r.ready), (1, 0));
        assert_eq!(r.language, ["name", "glottocode", "iso639_3"]);
        // The tutorial has its recording; «sɪks» is left to transcribe.
        let t = &r.texts[0];
        assert!(!t.missing.contains(&"recording".to_string()), "{t:?}");
        assert_eq!(&t.missing[..5], ["date", "place", "speakers", "genre", "description"]);
        assert!(t.missing.contains(&"consent".to_string()));
        assert!(t.missing.contains(&"transcription".to_string()), "{t:?}");
        // Details from the text or its recording, and a consent record, fill them in.
        let p = session::project(&mut s).unwrap();
        p.set_project_language("Tutorial English", "stan1293", "eng").unwrap();
        for (k, v) in [("speakers", "J. C."), ("genre", "counting"), ("description", "Counting to six.")] {
            p.set_text_metadata(&text, k, v).unwrap();
        }
        let (source, _) = p.text_recording(&text).unwrap().unwrap();
        p.set_source_detail(&source, "date", "2026-11-03").unwrap();
        p.set_source_detail(&source, "location", "Home").unwrap();
        let person = crate::people::add_person(&mut s, "J. C.", "speaker").unwrap().people[0].id.clone();
        crate::people::add_consent(&mut s, &person, &text, "2026-11-03").unwrap();
        let r = readiness(&mut s).unwrap();
        assert!(r.language.is_empty());
        let t = &r.texts[0];
        assert!(!t.missing.iter().any(|m| ["date", "place", "speakers", "genre", "description", "consent"].contains(&m.as_str())), "{t:?}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
