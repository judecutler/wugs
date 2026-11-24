//! A `.zenodo.json` beside a published dataset: the deposit metadata Zenodo
//! reads (its GitHub integration reads the file as is, and the same fields
//! fill its upload form), so a CLDF export goes to Zenodo with its title,
//! authors, description, keywords and language already written. Authors are
//! the People panel's researchers; never the speakers, whose naming is
//! theirs to decide. Fields wugs cannot know (the licence) are left out
//! and named in the export's notes.

use serde_json::{json, Value};
use wugs_core::Project;

/// "Ana Sefo" → "Sefo, Ana", the order Zenodo asks for; a single name stays as it is.
pub fn cited_name(full: &str) -> String {
    let parts: Vec<&str> = full.split_whitespace().collect();
    match parts.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{last}, {}", rest.join(" ")),
        _ => full.trim().to_string(),
    }
}

/// The roles whose people are a dataset's authors.
pub const AUTHOR_ROLES: &[&str] = &["researcher", "linguist", "depositor", "author"];

/// The record for a dataset: what it is (`what`, e.g. "CLDF Dictionary"),
/// a sentence of contents, and extra keywords.
pub fn record(p: &Project, what: &str, contents: &str, keywords: &[&str]) -> Result<(String, Vec<String>), String> {
    let lang = p.project_language()?;
    let project = p.setting(wugs_seal::PROJECT_TITLE)?.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let language = if lang.name.trim().is_empty() { "the language".to_string() } else { lang.name.trim().to_string() };
    let title = match &project {
        Some(t) => format!("{t}: {language} {what}"),
        None => format!("{} {what}", if lang.name.trim().is_empty() { "A" } else { lang.name.trim() }),
    };
    let mut notes = Vec::new();
    let mut creators: Vec<Value> = p
        .people()?
        .into_iter()
        .filter(|r| !r.deleted && r.roles.iter().any(|role| AUTHOR_ROLES.iter().any(|a| role.eq_ignore_ascii_case(a))))
        .map(|r| json!({ "name": cited_name(&r.name) }))
        .collect();
    if creators.is_empty() {
        creators.push(json!({ "name": "Surname, Given name" }));
        notes.push("No one in the People panel has the role researcher, so .zenodo.json names a placeholder author: write the authors in before depositing.".to_string());
    }
    let code = if lang.glottocode.is_empty() { String::new() } else { format!(" (Glottocode {})", lang.glottocode) };
    let description = format!("{language}{code}: {contents} Made with wugs, a toolkit for language documentation.");
    let mut words: Vec<String> = vec!["linguistics".into(), "language documentation".into()];
    if !lang.name.trim().is_empty() {
        words.push(lang.name.trim().to_string());
    }
    if !lang.glottocode.is_empty() {
        words.push(lang.glottocode.clone());
    }
    words.extend(keywords.iter().map(|k| k.to_string()));
    let mut rec = json!({
        "title": title,
        "upload_type": "dataset",
        "description": description,
        "creators": creators,
        "keywords": words,
    });
    if !lang.iso.is_empty() {
        // Zenodo takes a record's language as ISO 639-3.
        rec["language"] = json!(lang.iso);
    }
    notes.push("Choose a licence on Zenodo (or add \"license\" to .zenodo.json): wugs does not know which one the people and the community agreed to.".to_string());
    Ok((serde_json::to_string_pretty(&rec).map_err(|e| e.to_string())? + "\n", notes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session;

    #[test]
    fn a_record_names_the_researchers_and_never_the_speakers() {
        assert_eq!(cited_name("Ana Maria Sefo"), "Sefo, Ana Maria");
        assert_eq!(cited_name("Sefo"), "Sefo");
        let dir = std::env::temp_dir().join(format!("wugs-zenodo-{}", wugs_model::ulid()));
        let mut s: session::Session = None;
        session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let p = session::project(&mut s).unwrap();
        let (json, notes) = record(p, "CLDF Dictionary", "3 entries.", &["CLDF"]).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["title"], "A CLDF Dictionary");
        assert_eq!(v["creators"][0]["name"], "Surname, Given name");
        assert!(v.get("language").is_none());
        assert_eq!(notes.len(), 2);
        p.set_project_language("Kanu", "kanu1234", "knu").unwrap();
        p.set_setting(wugs_seal::PROJECT_TITLE, "Kanu documentation").unwrap();
        p.add_named_person("Ana Sefo", &["researcher".to_string()]).unwrap();
        p.add_named_person("Lena Okafor", &["speaker".to_string()]).unwrap();
        let (json, notes) = record(p, "CLDF Dictionary", "3 entries.", &["CLDF"]).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["title"], "Kanu documentation: Kanu CLDF Dictionary");
        assert_eq!(v["creators"], json!([{ "name": "Sefo, Ana" }]));
        assert_eq!(v["language"], "knu");
        assert!(v["description"].as_str().unwrap().starts_with("Kanu (Glottocode kanu1234): 3 entries."));
        assert!(v["keywords"].as_array().unwrap().iter().any(|k| k == "CLDF"));
        assert!(!json.contains("Okafor"), "speakers are never named");
        assert_eq!(notes.len(), 1);
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
