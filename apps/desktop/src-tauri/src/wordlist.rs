//! Export the Elicit panel's word lists as a CLDF Wordlist with an EDICTOR
//! table beside it (`wugs_cldf::wordlist`), each form cut into segments
//! by the project's orthography profile when it has one.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::session::{self, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordlistExport {
    pub path: String,
    pub lists: usize,
    pub concepts: usize,
    pub answered: usize,
    pub forms: usize,
    pub skipped: usize,
    /// Whether the orthography profile cut the segments (else letter by letter).
    pub by_profile: bool,
    pub notes: Vec<String>,
}

/// Write the word list (one list, or all) into a new folder inside `folder`.
pub fn export(s: &mut Session, folder: &Path, date: &str, list: Option<&str>, generator: &str) -> Result<WordlistExport, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    crate::interchange::outside_project(s, folder)?;
    let p = session::project(s)?;
    let lang = p.project_language()?;
    let title = if lang.name.is_empty() { "Word list".to_string() } else { format!("{} word list", lang.name) };
    let (segment, by_profile) = crate::orthography::segmenter(p)?;
    let wl = wugs_cldf::export_wordlist(p, &wugs_cldf::WordlistOptions { title: &title, generator, list, segment: &*segment })?;
    let date = crate::corrections::date_or_today(date);
    let mut out = folder.join(format!("wugs-wordlist-{date}"));
    let mut n = 2;
    while out.symlink_metadata().is_ok() {
        out = folder.join(format!("wugs-wordlist-{date}-{n}"));
        n += 1;
    }
    let scratch = folder.join(format!(".{}.partial", out.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let written = (|| {
        fs::create_dir(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        for (name, contents) in &wl.files {
            fs::write(scratch.join(name), contents).map_err(|e| format!("{name}: {e}"))?;
        }
        fs::rename(&scratch, &out).map_err(|e| format!("{}: {e}", out.display()))
    })();
    if let Err(e) = written {
        fs::remove_dir_all(&scratch).ok();
        return Err(e);
    }
    Ok(WordlistExport { path: out.display().to_string(), lists: wl.lists, concepts: wl.concepts, answered: wl.answered, forms: wl.forms, skipped: wl.skipped, by_profile, notes: wl.notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loaded_list_exports_beside_the_project_never_inside_it() {
        let dir = std::env::temp_dir().join(format!("wugs-wordlist-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir.join("project"), "tester").unwrap();
        let swadesh = crate::elicit::concept_lists_bundled(&mut session).unwrap().into_iter().next().unwrap();
        let list = crate::elicit::concept_list_load(&mut session, &swadesh.source).unwrap();
        let first = crate::elicit::concept_list_view(&mut session, &list.id).unwrap().concepts[0].id.clone();
        session::project(&mut session).unwrap().elicit(&first, "tʰaːm", "", None).unwrap();
        let outside = dir.join("out");
        std::fs::create_dir_all(&outside).unwrap();
        let done = export(&mut session, &outside, "2026-11-23", None, "wugs test").unwrap();
        assert_eq!((done.forms, done.answered, done.by_profile), (1, 1, false));
        assert!(done.path.ends_with("wugs-wordlist-2026-11-23"));
        let forms = std::fs::read_to_string(Path::new(&done.path).join("forms.csv")).unwrap();
        assert!(forms.contains(",tʰaːm,tʰaːm,tʰ aː m,"), "{forms}");
        assert!(Path::new(&done.path).join("edictor.tsv").is_file());
        assert!(export(&mut session, &dir.join("project"), "2026-11-23", None, "x").is_err(), "never inside the project");
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
