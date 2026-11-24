//! Sentences written twice (the Checks panel's Sentences written twice): the
//! same words in the same order in two or more segments, case ignored, as a
//! text imported twice, a sentence pasted again or a web page saved from two
//! addresses leave them; corpus counts take each copy as a separate use. Read
//! through the corpus tools' one reader; nothing is written.

use std::collections::HashMap;

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{project, Session};

/// Sentences shorter than this (in words) are left out: short replies repeat by nature.
pub const MIN_WORDS: usize = 4;
/// At most this many groups are listed; `groups` counts every one.
const MAX_LISTED: usize = 300;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Copy {
    pub text_id: String,
    pub text_title: String,
    pub tier_id: String,
    pub segment_id: String,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Twice {
    /// The sentence as its first copy writes it.
    pub sentence: String,
    pub copies: Vec<Copy>,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Duplicates {
    /// The most copied first, at most `MAX_LISTED`.
    pub listed: Vec<Twice>,
    /// Every sentence written more than once.
    pub groups: usize,
    /// Every copy beyond the first, the uses corpus counts take twice.
    pub extra: usize,
    /// Sentences read (of `MIN_WORDS` words or more).
    pub read: usize,
}

pub fn duplicates(session: &mut Session) -> Result<Duplicates, String> {
    let p = project(session)?;
    let mut by_key: HashMap<String, Twice> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut read = 0;
    each_segment(p, Read::Words, &Scope::all(), |seg| {
        let keys: Vec<String> = seg.words.iter().map(|w| wugs_model::fold_key(&p.proj.nfc(w))).filter(|k| !k.is_empty()).collect();
        if keys.len() < MIN_WORDS {
            return Ok(());
        }
        read += 1;
        let key = keys.join("\u{1f}");
        let copy = Copy { text_id: seg.text.clone(), text_title: seg.title.clone(), tier_id: seg.tier.clone(), segment_id: seg.id.clone() };
        match by_key.get_mut(&key) {
            Some(t) => t.copies.push(copy),
            None => {
                order.push(key.clone());
                by_key.insert(key, Twice { sentence: seg.words.join(" "), copies: vec![copy] });
            }
        }
        Ok(())
    })?;
    let mut twice: Vec<Twice> = order.into_iter().filter_map(|k| by_key.remove(&k)).filter(|t| t.copies.len() > 1).collect();
    let groups = twice.len();
    let extra = twice.iter().map(|t| t.copies.len() - 1).sum();
    // Most copies first; the sort is stable, so ties keep the texts' order.
    twice.sort_by_key(|t| std::cmp::Reverse(t.copies.len()));
    twice.truncate(MAX_LISTED);
    Ok(Duplicates { listed: twice, groups, extra, read })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session;

    #[test]
    fn the_same_words_in_two_places_are_found_once_each() {
        let dir = std::env::temp_dir().join(format!("wugs-dup-test-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        let p = project(&mut s).unwrap();
        let tier = session::find_utterance_tier(p).unwrap().unwrap();
        p.import_written_text("Fish", &tier, &["kuri mopa tela senu", "waka lolo pinu", "Kuri  mopa tela senu."]).unwrap();
        p.import_written_text("Fish again", &tier, &["kuri mopa tela senu", "waka lolo pinu sela"]).unwrap();
        let d = duplicates(&mut s).unwrap();
        assert_eq!(d.groups, 1, "{d:?}");
        assert_eq!(d.extra, 2);
        assert_eq!(d.listed[0].copies.len(), 3, "case, spacing and punctuation ignored");
        assert_eq!(d.listed[0].sentence, "kuri mopa tela senu");
        assert_eq!(d.listed[0].copies[2].text_title, "Fish again");
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
