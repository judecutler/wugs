//! Frequency list (Scrolls, SCROLLS.md §5 step 3, first form): how often each
//! word, morph, gloss or translation word occurs, in the whole project or one
//! text, read the way the Concordance reads (`concordance::each_segment`), so
//! a row's count is the number of lines the Concordance shows for it when
//! asked for that exact item in the same text or the whole project.
//!
//! A morph or gloss counts once per word that has it, since the Concordance
//! shows one line per word. Items are counted under the Concordance's keys: words and translation
//! words by `fold_key` (case folded, diacritics kept), morphs and glosses by
//! `lexicon_key` (edge markers dropped too). Each row shows the spelling met
//! most often (the first met on a tie), so `Kesi` and `kesi` count together
//! under whichever the texts use more.
//!
//! Cost: one pass over every segment, token and morph, like a concordance
//! query; the typed `tokens` table (SCROLLS.md §2) replaces it for large corpora.

use std::collections::HashMap;

use serde::Serialize;

use crate::concordance::{bare, each_segment, morphs_of, Kind};
use crate::session::{project, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyRow {
    /// The spelling met most often.
    pub item: String,
    pub count: usize,
    /// How many texts it occurs in.
    pub texts: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frequencies {
    /// Most frequent first; ties in the order the items were first met.
    pub rows: Vec<FrequencyRow>,
    /// Every occurrence counted (tokens).
    pub tokens: usize,
    /// Distinct items (types), even past the limit.
    pub types: usize,
}

/// At most this many rows are returned; `types` says how many there were.
pub const LIMIT: usize = 5000;

#[derive(Default)]
struct Tally {
    first: usize,
    count: usize,
    spellings: Vec<(String, usize)>,
    texts: Vec<String>,
}

pub fn frequencies(session: &mut Session, kind: Kind, text: Option<&str>) -> Result<Frequencies, String> {
    if kind == Kind::Sense {
        return Err("a frequency list counts words, morphs, glosses or translation words".into());
    }
    let p = project(session)?;
    let mut tallies: HashMap<String, Tally> = HashMap::new();
    let mut tokens = 0usize;
    let mut count = |key: String, spelling: &str, text: &str, tokens: &mut usize| {
        if key.is_empty() {
            return;
        }
        *tokens += 1;
        let next = tallies.len();
        let t = tallies.entry(key).or_insert_with(|| Tally { first: next, ..Tally::default() });
        t.count += 1;
        match t.spellings.iter_mut().find(|(s, _)| s == spelling) {
            Some((_, n)) => *n += 1,
            None => t.spellings.push((spelling.to_string(), 1)),
        }
        if t.texts.last().map(String::as_str) != Some(text) && !t.texts.iter().any(|x| x == text) {
            t.texts.push(text.to_string());
        }
    };
    each_segment(p, kind == Kind::Translation, text, |seg| {
        match kind {
            Kind::Word | Kind::Translation => {
                for w in &seg.words {
                    let spelling = p.proj.nfc(w);
                    count(wugs_model::fold_key(&spelling), &spelling, &seg.text, &mut tokens);
                }
            }
            Kind::Morph | Kind::Gloss => {
                // Once per word, as the Concordance shows one line per word:
                // `lo~lo` or two PL glosses in one word count once.
                for token in &seg.tokens {
                    let mut seen = std::collections::HashSet::new();
                    for m in morphs_of(p, token)? {
                        let raw = if kind == Kind::Morph { m.surface } else { m.gloss };
                        let spelling = p.proj.nfc(bare(&raw));
                        let key = wugs_model::lexicon_key(&spelling);
                        if seen.insert(key.clone()) {
                            count(key, &spelling, &seg.text, &mut tokens);
                        }
                    }
                }
            }
            Kind::Sense => unreachable!("refused above"),
        }
        Ok(())
    })?;
    let types = tallies.len();
    let mut all: Vec<Tally> = tallies.into_values().collect();
    all.sort_by(|a, b| b.count.cmp(&a.count).then(a.first.cmp(&b.first)));
    all.truncate(LIMIT);
    let rows = all
        .into_iter()
        .map(|t| {
            let item = t.spellings.iter().fold(None::<&(String, usize)>, |best, s| if best.is_none_or(|b| s.1 > b.1) { Some(s) } else { best }).map(|s| s.0.clone()).unwrap_or_default();
            FrequencyRow { item, count: t.count, texts: t.texts.len() }
        })
        .collect();
    Ok(Frequencies { rows, tokens, types })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_the_sample_like_the_concordance_finds_it() {
        let dir = std::env::temp_dir().join(format!("wugs-freq-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        // A segment with a glottal stop written `?` and a reduplicated word
        // whose two morphs share a gloss: the cases a count can get wrong.
        let text = crate::session::list_texts(&mut session).unwrap()[0].id.clone();
        let seg = crate::session::add_segment(&mut session, &text, None, Some(19_000.0), Some(19_900.0), "ka?a kaba lolo").unwrap().id;
        let forms: Vec<String> = ["ka?a", "kaba", "lolo"].iter().map(|s| s.to_string()).collect();
        let view = crate::gloss::gloss_tokenize(&mut session, &text, &seg, &forms).unwrap().view;
        let m = |surface: &str, kind: &str| crate::gloss::MorphSpecView { surface: surface.into(), morph_type: kind.into(), gloss: Some("PL".into()), glosses_by_lang: Default::default(), sense: None };
        crate::gloss::gloss_apply(&mut session, &text, &seg, &view.words[2].id, vec![m("lo", "reduplication"), m("lo", "root")]).unwrap();
        let words = frequencies(&mut session, Kind::Word, None).unwrap();
        assert!(words.tokens > 0 && words.types <= words.tokens);
        // Every row's count is what the Concordance finds for that exact item,
        // in the whole project and in the one text.
        for kind in [Kind::Word, Kind::Morph, Kind::Gloss, Kind::Translation] {
            for scope in [None, Some(text.as_str())] {
                for row in frequencies(&mut session, kind, scope).unwrap().rows {
                    let found = crate::concordance::concordance_in(&mut session, &row.item, kind, crate::concordance::Match::Exact, scope).unwrap().total;
                    assert_eq!(found, row.count, "{kind:?} {}", row.item);
                }
            }
        }
        let lo = frequencies(&mut session, Kind::Morph, None).unwrap().rows.into_iter().find(|r| r.item == "lo").unwrap();
        assert_eq!(lo.count, 1, "a word with lo twice counts once");
        assert!(words.rows.windows(2).all(|w| w[0].count >= w[1].count), "most frequent first");
        let morphs = frequencies(&mut session, Kind::Morph, None).unwrap();
        let kani = morphs.rows.iter().find(|r| r.item == "kani").expect("kani counted");
        assert_eq!(kani.count, 1);
        let ni = morphs.rows.iter().find(|r| r.item == "ni").expect("markers dropped");
        assert_eq!(ni.texts, 1);
        assert!(frequencies(&mut session, Kind::Gloss, None).unwrap().rows.iter().any(|r| r.item == "1SG"));
        let t = frequencies(&mut session, Kind::Translation, None).unwrap();
        assert!(t.rows.iter().any(|r| r.item.eq_ignore_ascii_case("three")));
        // One text: the sample has one, so it is the whole project.
        assert_eq!(frequencies(&mut session, Kind::Word, Some(&text)).unwrap().tokens, words.tokens);
        assert_eq!(frequencies(&mut session, Kind::Word, Some("txt_none")).unwrap().tokens, 0);
        assert!(frequencies(&mut session, Kind::Sense, None).is_err());
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
