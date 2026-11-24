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
}
