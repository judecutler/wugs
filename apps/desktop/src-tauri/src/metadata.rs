//! Document metadata for the corpus tools (SCROLLS.md §2): read and edit a
//! text's `meta:<key>` fields, and bring them in from a table beside the
//! files (a CSV or TSV with one row per document). The core keeps the
//! fields (`wugs_core::metadata`); this module is the shell's side.

use std::path::Path;

use serde::Serialize;

use crate::session::{self, Session};

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MetaField {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MetaKey {
    pub key: String,
    /// How many texts have it.
    pub texts: usize,
}

/// One text's metadata, and every key the project uses (for suggestions).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaView {
    pub fields: Vec<MetaField>,
    pub keys: Vec<MetaKey>,
}

fn view(s: &mut Session, text_id: &str) -> Result<MetaView, String> {
    let p = session::project(s)?;
    let fields = p.text_metadata(text_id)?.into_iter().map(|(key, value)| MetaField { key, value }).collect();
    let keys = p.metadata_keys()?.into_iter().map(|(key, texts)| MetaKey { key, texts }).collect();
    Ok(MetaView { fields, keys })
}

pub fn text_metadata(s: &mut Session, text_id: &str) -> Result<MetaView, String> {
    view(s, text_id)
}

/// Set one key (an empty value clears it); returns the text's view.
pub fn set_text_metadata(s: &mut Session, text_id: &str, key: &str, value: &str) -> Result<MetaView, String> {
    session::project(s)?.set_text_metadata(text_id, key, value)?;
    view(s, text_id)
}

/// One key's values across the project, for picking texts by a detail.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MetaValues {
    pub key: String,
    /// Each value with how many texts have it, most texts first, then by value.
    pub values: Vec<(String, usize)>,
}

/// Every key the project's live texts use, by key, each with its values.
pub fn metadata_values(s: &mut Session) -> Result<Vec<MetaValues>, String> {
    let p = session::project(s)?;
    let mut by: std::collections::BTreeMap<String, std::collections::BTreeMap<String, usize>> = Default::default();
    for text in p.proj.entities_of_kind("text")? {
        for (k, v) in p.text_metadata(&text)? {
            *by.entry(k).or_default().entry(v).or_default() += 1;
        }
    }
    Ok(by
        .into_iter()
        .map(|(key, values)| {
            let mut values: Vec<(String, usize)> = values.into_iter().collect();
            values.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            MetaValues { key, values }
        })
        .collect())
}

/// What a metadata table brought in.
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MetaImport {
    /// The column that named the documents.
    pub by: String,
    /// The other columns, as the keys they were written under.
    pub keys: Vec<String>,
    /// Texts that got at least one value written.
    pub texts: usize,
    /// Values written (a value a text already had is not written again).
    pub fields: usize,
    /// Rows whose document matched no text, as named in the table (at most 20).
    pub unmatched: Vec<String>,
    pub unmatched_count: usize,
    /// Rows that named a title several texts share; each of them got the values.
    pub shared: usize,
}

/// Columns that name the document a row is about, most preferred first.
const NAMES: [&str; 9] = ["title", "text", "document", "doc", "file", "filename", "name", "text_id", "id"];

/// The ways a row's document name may match a text: its title as written,
/// without a file extension, and as `file_stem` writes it (so a list of the
/// exported files' names matches too), all compared case-blind in NFC.
fn forms(p: &wugs_core::Project, name: &str) -> Vec<String> {
    let name = p.proj.nfc(name.trim());
    let mut out = vec![name.to_lowercase()];
    let bare = Path::new(&name).file_stem().and_then(|s| s.to_str()).map(str::to_lowercase);
    if let Some(b) = bare.filter(|b| !out.contains(b)) {
        out.push(b);
    }
    out
}

/// Bring in a table of document metadata: a header row, one row per
/// document; the document column is the first header named title, text,
/// document, doc, file, filename, name, text_id or id (else the first
/// column), every other column a key. Rows are matched to texts by title
/// (case-blind, with or without a file extension, or the exported file's
/// name). One txn for the whole table; an empty cell writes nothing.
pub fn import_metadata(s: &mut Session, path: &Path) -> Result<MetaImport, String> {
    let (text, tabs) = crate::table_text::read_table(path, &["tsv", "txt", "tab"])?;
    let (header, rows, _) = crate::table_text::records(&text, tabs)?;
    let named = |wanted: &str| header.iter().position(|h| wugs_core::metadata::key_of(h).as_deref() == Some(wanted));
    let by = NAMES.iter().find_map(|n| named(n)).unwrap_or(0);
    let columns: Vec<(usize, String)> = header.iter().enumerate().filter(|&(i, _)| i != by).filter_map(|(i, h)| Some((i, wugs_core::metadata::key_of(h)?))).collect();
    if columns.is_empty() {
        return Err(format!("the table has no column besides «{}» to bring in", header[by]));
    }
    let p = session::project(s)?;
    let mut index: std::collections::HashMap<String, Vec<String>> = Default::default();
    for id in p.proj.entities_of_kind("text")? {
        let title = p.proj.field_str(&id, "title")?.unwrap_or_default();
        let mut keys = forms(p, &title);
        keys.push(crate::interchange::file_stem(&title, "text").to_lowercase());
        keys.dedup();
        for k in keys {
            let ids = index.entry(k).or_default();
            if !ids.contains(&id) {
                ids.push(id.clone());
            }
        }
    }
    let mut report = MetaImport { by: header[by].clone(), keys: columns.iter().map(|(_, k)| k.clone()).collect(), ..Default::default() };
    let mut writes: Vec<(String, String, String)> = Vec::new();
    for row in &rows {
        let name = row[by].trim();
        if name.is_empty() {
            continue;
        }
        let Some(ids) = forms(p, name).iter().find_map(|f| index.get(f)) else {
            report.unmatched_count += 1;
            if report.unmatched.len() < 20 {
                report.unmatched.push(name.to_string());
            }
            continue;
        };
        if ids.len() > 1 {
            report.shared += 1;
        }
        for id in ids {
            for (i, key) in &columns {
                writes.push((id.clone(), key.clone(), row[*i].clone()));
            }
        }
    }
    let before: std::collections::BTreeSet<String> = writes.iter().map(|(t, _, _)| t.clone()).collect();
    let changed: std::collections::BTreeSet<String> = writes
        .iter()
        .filter(|(t, k, v)| !v.trim().is_empty() && p.proj.field_str(t, &format!("{}{k}", wugs_core::metadata::PREFIX)).ok().flatten().as_deref() != Some(v.trim()))
        .map(|(t, _, _)| t.clone())
        .collect();
    let (fields, _) = p.import_text_metadata(&writes)?;
    report.fields = fields;
    report.texts = changed.intersection(&before).count();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn a_table_beside_the_files_fills_in_each_text() {
        let dir = std::env::temp_dir().join(format!("wugs-meta-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let p = session::project(&mut s).unwrap();
        let a = p.create_text("Market talk").unwrap();
        let b = p.create_text("The fox: a story").unwrap();
        fs::create_dir_all(&dir).unwrap();
        let table = dir.join("metadata.csv");
        // `The fox_ a story.txt` is the exported file's name for «The fox: a story».
        fs::write(&table, "File,Genre,Speaker,Year\nmarket talk.eaf,conversation,A,2024\nThe fox_ a story.txt,narrative,,2023\nNot here,x,y,z\n").unwrap();
        let r = import_metadata(&mut s, &table).unwrap();
        assert_eq!((r.by.as_str(), r.keys.clone()), ("File", vec!["genre".to_string(), "speaker".into(), "year".into()]));
        assert_eq!((r.texts, r.fields, r.unmatched.clone()), (2, 5, vec!["Not here".to_string()]));
        let v = text_metadata(&mut s, &a).unwrap();
        assert_eq!(v.fields, vec![MetaField { key: "genre".into(), value: "conversation".into() }, MetaField { key: "speaker".into(), value: "A".into() }, MetaField { key: "year".into(), value: "2024".into() }]);
        assert_eq!(v.keys.iter().find(|k| k.key == "genre").map(|k| k.texts), Some(2));
        let again = import_metadata(&mut s, &table).unwrap();
        assert_eq!((again.texts, again.fields), (0, 0), "the same table again writes nothing");
        let values = metadata_values(&mut s).unwrap();
        assert_eq!(values.iter().find(|m| m.key == "genre").map(|m| m.values.clone()), Some(vec![("conversation".to_string(), 1), ("narrative".to_string(), 1)]));
        let v = set_text_metadata(&mut s, &b, "Genre", "fable").unwrap();
        assert!(v.fields.contains(&MetaField { key: "genre".into(), value: "fable".into() }));
        drop(s);
        fs::remove_dir_all(dir).ok();
    }
}
