//! Semantic domains a sense's gloss suggests: SIL's elicitation questions
//! list the example words of each domain ("What words refer to the sky? sky,
//! firmament, canopy…"), so a gloss that is one of a domain's example words
//! points at that domain. Offered in the Lexicon's domain picker; never
//! assigned unasked.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use serde::Serialize;

use crate::lexicon::SIL_SEMDOM_V4;
use crate::session::{project, Session};

/// The most suggestions offered for one gloss.
const MAX: usize = 5;

/// A gloss or an example word as compared: lower case, a leading "to " (verbs)
/// and "a "/"the " left out, brackets dropped, spaces collapsed.
pub fn key(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let lower = out.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = lower.trim_matches(|c: char| !c.is_alphanumeric());
    ["to ", "a ", "an ", "the "].iter().fold(trimmed.to_string(), |s, p| s.strip_prefix(p).map(str::to_string).unwrap_or(s))
}

/// Example word → the codes of the domains listing it, in outline order.
fn index() -> &'static HashMap<String, Vec<String>> {
    static INDEX: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut idx: HashMap<String, Vec<String>> = HashMap::new();
        for line in SIL_SEMDOM_V4.lines().filter(|l| !l.trim().is_empty()) {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            let Some(code) = v["code"].as_str() else { continue };
            let mut words: Vec<String> = Vec::new();
            if let Some(name) = v["name"].as_str() {
                words.extend(name.split(',').map(key));
            }
            for q in v["questions"].as_array().into_iter().flatten() {
                if let Some(list) = q.get(1).and_then(|w| w.as_str()) {
                    words.extend(list.split([',', ';']).map(key));
                }
            }
            for w in words.into_iter().filter(|w| !w.is_empty()) {
                let codes = idx.entry(w).or_default();
                if !codes.iter().any(|c| c == code) {
                    codes.push(code.to_string());
                }
            }
        }
        idx
    })
}

/// The domain codes a gloss suggests: the whole gloss first, then each of its
/// parts (`dog; hound`), each a domain's example word; deeper domains first
/// within a part, since they say more; at most `MAX`.
pub fn suggest(gloss: &str) -> Vec<String> {
    let idx = index();
    let mut out: Vec<String> = Vec::new();
    let whole = key(gloss);
    let parts = std::iter::once(whole.clone()).chain(gloss.split([';', ',', '/']).map(key)).filter(|p| !p.is_empty());
    for part in parts {
        let Some(codes) = idx.get(&part) else { continue };
        let mut codes = codes.clone();
        codes.sort_by_key(|c| std::cmp::Reverse(c.split('.').count()));
        for c in codes {
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out.truncate(MAX);
    out
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggested {
    pub id: String,
    pub code: String,
    pub label: String,
}

/// The project's domains the gloss suggests (SIL's codes found among them), in order.
pub fn suggested(session: &mut Session, gloss: &str) -> Result<Vec<Suggested>, String> {
    let codes = suggest(gloss);
    if codes.is_empty() {
        return Ok(Vec::new());
    }
    let p = project(session)?;
    let mut by_code: BTreeMap<String, (String, String)> = BTreeMap::new();
    for id in p.proj.entities_of_kind("domain")? {
        if p.proj.is_tombstoned(&id)? {
            continue;
        }
        let code = p.proj.field_str(&id, "code")?.unwrap_or_default();
        if codes.contains(&code) {
            by_code.entry(code).or_insert((id.clone(), p.proj.field_str(&id, "label")?.unwrap_or_default()));
        }
    }
    Ok(codes.into_iter().filter_map(|c| by_code.get(&c).map(|(id, label)| Suggested { id: id.clone(), code: c, label: label.clone() })).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gloss_finds_the_domains_listing_it() {
        assert_eq!(key("to run (fast)"), "run");
        assert_eq!(key("The Sky."), "sky");
        let sky = suggest("sky");
        assert_eq!(sky.first().map(String::as_str), Some("1.1"), "{sky:?}");
        let dog = suggest("dog");
        assert!(dog.iter().any(|c| c.starts_with("1.6")), "an animal's domain: {dog:?}");
        assert!(suggest("hound; dog").len() <= MAX);
        assert!(suggest("zxqv").is_empty());
    }

    #[test]
    fn suggestions_are_the_projects_own_domains() {
        let dir = std::env::temp_dir().join(format!("wugs-domsug-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        assert!(suggested(&mut s, "sky").unwrap().is_empty(), "no domains loaded yet");
        crate::lexicon::domains_load_sil(&mut s).unwrap();
        let got = suggested(&mut s, "sky").unwrap();
        assert_eq!((got[0].code.as_str(), got[0].label.as_str()), ("1.1", "Sky"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
