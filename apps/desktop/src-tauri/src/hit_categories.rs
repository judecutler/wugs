//! Sorting a search's lines into categories (CQPweb's "categorise query",
//! the qualitative half of corpus work): the person names categories for a
//! query ("literal, figurative, unclear") and puts each Concordance line in
//! one. Each line's category is its own project setting, so two people
//! sorting the same lines on two devices never overwrite each other's work,
//! and the categories go with the project.
//!
//! Keys: `hit-categories:<query key>` holds the category names, one a line;
//! `hit-category:<query key>:<segment>:<at>` a line's category. The query
//! key is a hash of the search kind and the query as typed.

use serde::Serialize;

use crate::session::{self, Session};

/// At most this many categories per query.
const MAX_CATEGORIES: usize = 20;
/// The longest category name (characters).
const MAX_NAME: usize = 40;

fn query_key(kind: &str, query: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in kind.bytes().chain([0x1f]).chain(query.trim().bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn line_key(kind: &str, query: &str, segment: &str, at: usize) -> Result<String, String> {
    if segment.is_empty() || !segment.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        return Err("That line names no segment.".into());
    }
    Ok(format!("hit-category:{}:{segment}:{at}", query_key(kind, query)))
}

/// A query's categories and the lines sorted so far (`<segment>:<at>` → category).
#[derive(Debug, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Sorted {
    pub categories: Vec<String>,
    pub lines: Vec<(String, String)>,
}

pub fn get(session: &mut Session, kind: &str, query: &str) -> Result<Sorted, String> {
    let p = session::project(session)?;
    let key = query_key(kind, query);
    let categories = p.setting(&format!("hit-categories:{key}"))?.unwrap_or_default().lines().map(str::to_string).filter(|c| !c.is_empty()).collect();
    let prefix = format!("hit-category:{key}:");
    let mut lines: Vec<(String, String)> = p.settings_with_prefix(&prefix)?.into_iter().map(|(k, v)| (k[prefix.len()..].to_string(), v)).collect();
    lines.sort();
    Ok(Sorted { categories, lines })
}

/// Name a query's categories (NFC, spaces collapsed, repeats and empty ones dropped).
pub fn set_categories(session: &mut Session, kind: &str, query: &str, categories: &[String]) -> Result<Sorted, String> {
    let p = session::project(session)?;
    let mut clean: Vec<String> = Vec::new();
    for c in categories {
        let c = p.proj.nfc(&c.split_whitespace().collect::<Vec<_>>().join(" "));
        if c.is_empty() || clean.contains(&c) {
            continue;
        }
        if c.chars().count() > MAX_NAME {
            return Err(format!("A category's name is at most {MAX_NAME} characters."));
        }
        clean.push(c);
    }
    if clean.len() > MAX_CATEGORIES {
        return Err(format!("A search can have at most {MAX_CATEGORIES} categories."));
    }
    let key = format!("hit-categories:{}", query_key(kind, query));
    let value = clean.join("\n");
    if p.setting(&key)?.unwrap_or_default() != value {
        p.set_setting(&key, &value)?;
    }
    get(session, kind, query)
}

/// Put one line in a category (empty takes it out of any).
pub fn set_line(session: &mut Session, kind: &str, query: &str, segment: &str, at: usize, category: &str) -> Result<Sorted, String> {
    let key = line_key(kind, query, segment, at)?;
    let p = session::project(session)?;
    let category = p.proj.nfc(category.trim());
    if p.setting(&key)?.unwrap_or_default() != category {
        p.set_setting(&key, &category)?;
    }
    get(session, kind, query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_sorted_into_a_query_s_categories() {
        let dir = std::env::temp_dir().join(format!("wugs-hitcat-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, dir.to_str().unwrap(), "tester").unwrap();
        assert_eq!(get(&mut s, "word", "eye").unwrap(), Sorted::default());
        let r = set_categories(&mut s, "word", "eye", &["literal".into(), " figurative ".into(), "literal".into(), "".into()]).unwrap();
        assert_eq!(r.categories, ["literal", "figurative"]);
        set_line(&mut s, "word", "eye", "seg01", 2, "figurative").unwrap();
        let r = set_line(&mut s, "word", "eye", "seg02", 0, "literal").unwrap();
        assert_eq!(r.lines, [("seg01:2".to_string(), "figurative".to_string()), ("seg02:0".to_string(), "literal".to_string())]);
        let r = set_line(&mut s, "word", "eye", "seg01", 2, "").unwrap();
        assert_eq!(r.lines.len(), 1, "an emptied line is out of every category");
        assert!(get(&mut s, "word", "eyes").unwrap().categories.is_empty(), "another query has its own");
        assert!(get(&mut s, "gloss", "eye").unwrap().categories.is_empty(), "and another kind");
        assert!(set_line(&mut s, "word", "eye", "a b", 0, "x").is_err());
        assert!(set_categories(&mut s, "word", "eye", &["x".repeat(41)]).is_err());
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
