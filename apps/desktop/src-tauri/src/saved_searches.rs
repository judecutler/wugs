//! Saved searches (CQPweb's "saved queries"): a Concordance search kept with
//! the project under a name, to run again later or on another device. Each
//! is its own project setting `saved-search:<FNV-1a of the folded name>`
//! holding JSON, so two people saving different searches never overwrite
//! each other, and saving under a name already used replaces that search.
//! An emptied setting reads as gone.

use serde::{Deserialize, Serialize};

use crate::concordance::Where;
use crate::session::{project, Session};

const PREFIX: &str = "saved-search:";
/// The longest name kept (characters).
const MAX_NAME: usize = 80;
/// The longest query kept (characters).
const MAX_QUERY: usize = 2000;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSearch {
    pub name: String,
    /// The Concordance's kind: `word`, `morph`, `gloss`, `translation` or `query`.
    pub kind: String,
    pub query: String,
    /// One text, when the search was in one (its title as it was, for the list).
    #[serde(default)]
    pub text_id: Option<String>,
    #[serde(default)]
    pub text_title: Option<String>,
    /// Met exactly (from Frequencies), else as a pattern.
    #[serde(default)]
    pub exact: bool,
    /// The texts a detail picks, or one track.
    #[serde(default)]
    pub within: Option<Where>,
}

const KINDS: [&str; 5] = ["word", "morph", "gloss", "translation", "query"];

fn key_of(name: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in wugs_model::fold_key(name).bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{PREFIX}{h:016x}")
}

/// Every saved search, by name.
pub fn list(session: &mut Session) -> Result<Vec<SavedSearch>, String> {
    let p = project(session)?;
    let mut out: Vec<SavedSearch> = p.settings_with_prefix(PREFIX)?.into_iter().filter_map(|(_, v)| serde_json::from_str(&v).ok()).collect();
    out.sort_by(|a, b| wugs_model::fold_key(&a.name).cmp(&wugs_model::fold_key(&b.name)));
    Ok(out)
}

/// Keep a search under its name (a name already used, capitals aside, is replaced).
pub fn save(session: &mut Session, search: SavedSearch) -> Result<Vec<SavedSearch>, String> {
    let p = project(session)?;
    let name = p.proj.nfc(&search.name.split_whitespace().collect::<Vec<_>>().join(" "));
    if name.is_empty() {
        return Err("Give the search a name.".into());
    }
    if name.chars().count() > MAX_NAME {
        return Err(format!("A search's name is at most {MAX_NAME} characters."));
    }
    if search.query.trim().is_empty() {
        return Err("There is no search to keep yet.".into());
    }
    if search.query.chars().count() > MAX_QUERY {
        return Err(format!("A search is at most {MAX_QUERY} characters."));
    }
    if !KINDS.contains(&search.kind.as_str()) {
        return Err(format!("{:?} is not a kind of search", search.kind));
    }
    let clean = SavedSearch { name: name.clone(), ..search };
    let value = serde_json::to_string(&clean).map_err(|e| e.to_string())?;
    p.set_setting(&key_of(&name), &value)?;
    list(session)
}

/// Forget a saved search.
pub fn delete(session: &mut Session, name: &str) -> Result<Vec<SavedSearch>, String> {
    let p = project(session)?;
    let key = key_of(name);
    if p.setting(&key)?.is_some_and(|v| !v.is_empty()) {
        p.set_setting(&key, "")?;
    }
    list(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_are_kept_by_name() {
        let dir = std::env::temp_dir().join(format!("wugs-saved-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let search = |name: &str, query: &str| SavedSearch { name: name.into(), kind: "query".into(), query: query.into(), text_id: None, text_title: None, exact: false, within: Some(Where::detail("genre", "news")) };
        assert!(save(&mut s, search("  ", "[upos=\"VERB\"]")).is_err());
        assert!(save(&mut s, search("Verbs", " ")).is_err());
        assert!(save(&mut s, SavedSearch { kind: "sense".into(), ..search("x", "y") }).is_err());
        save(&mut s, search(" Negation  verbs ", "[lemma=\"not\"] [upos=\"VERB\"]")).unwrap();
        let all = save(&mut s, search("Clitics", "*=*")).unwrap();
        assert_eq!(all.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), ["Clitics", "Negation verbs"]);
        assert_eq!(all[1].within, Some(Where::detail("genre", "news")));
        // The same name, capitals aside, replaces the search.
        let all = save(&mut s, search("negation verbs", "[lemma=\"no\"]")).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].query, "[lemma=\"no\"]");
        let all = delete(&mut s, "CLITICS").unwrap();
        assert_eq!(all.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), ["negation verbs"]);
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
