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
//! Cost: words and translation words come from the projection's corpus index
//! (one ordered read); morphs and glosses still read every token and morph.

use std::collections::HashMap;

use serde::Serialize;

use crate::concordance::{Read, bare, each_segment, morphs_of, Kind};
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
    /// Running words in the scope (equal to `tokens` for single words).
    pub words: usize,
    /// Sequences met only once, left out of `rows` (n-grams only).
    pub once: usize,
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
    frequencies_of(session, kind, text, 1)
}

/// The longest word sequence an n-gram list counts.
pub const MAX_N: usize = 5;

/// A frequency list of single items (`size` 1) or of word n-grams: runs of
/// `size` words in a row inside one segment (AntConc's N-Grams), counted
/// under their words' keys, words and translation words only. A sequence
/// row's count equals the Concordance's lines for it (an exact query of
/// several words meets them in a row). Sequences met only once are left out
/// (`once` says how many), as almost every sequence occurs once and would
/// crowd the list; `words` is the running word count rates are taken from.
pub fn frequencies_of(session: &mut Session, kind: Kind, text: Option<&str>, size: usize) -> Result<Frequencies, String> {
    if kind == Kind::Sense {
        return Err("a frequency list counts words, morphs, glosses or translation words".into());
    }
    if !(1..=MAX_N).contains(&size) {
        return Err(format!("n-grams are 1 to {MAX_N} words long"));
    }
    if size > 1 && !matches!(kind, Kind::Word | Kind::Translation) {
        return Err("n-grams are counted over words or translation words".into());
    }
    let p = project(session)?;
    let mut tallies: HashMap<String, Tally> = HashMap::new();
    let mut tokens = 0usize;
    let mut running = 0usize;
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
    each_segment(p, Read::of(kind), text, |seg| {
        match kind {
            Kind::Word | Kind::Translation => {
                let words: Vec<(String, String)> = seg
                    .words
                    .iter()
                    .map(|w| {
                        let spelling = p.proj.nfc(w);
                        (wugs_model::fold_key(&spelling), spelling)
                    })
                    .filter(|(k, _)| !k.is_empty())
                    .collect();
                running += words.len();
                for run in words.windows(size) {
                    // Keys joined by a character no word holds, so `a b`+`c` and `a`+`b c` stay apart.
                    let key = run.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join("\u{1f}");
                    let spelling = run.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join(" ");
                    count(key, &spelling, &seg.text, &mut tokens);
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
    let before = all.len();
    if size > 1 {
        all.retain(|t| t.count > 1);
    }
    let once = before - all.len();
    all.sort_by(|a, b| b.count.cmp(&a.count).then(a.first.cmp(&b.first)));
    all.truncate(LIMIT);
    let rows = all
        .into_iter()
        .map(|t| {
            let item = t.spellings.iter().fold(None::<&(String, usize)>, |best, s| if best.is_none_or(|b| s.1 > b.1) { Some(s) } else { best }).map(|s| s.0.clone()).unwrap_or_default();
            FrequencyRow { item, count: t.count, texts: t.texts.len() }
        })
        .collect();
    let words = if matches!(kind, Kind::Word | Kind::Translation) { running } else { tokens };
    Ok(Frequencies { rows, tokens, types, words, once })
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
        // Bigrams: one per pair of words in a row, never across segments.
        crate::session::add_segment(&mut session, &text, None, Some(20_000.0), Some(20_900.0), "Ka?a kaba").unwrap();
        let words = frequencies(&mut session, Kind::Word, None).unwrap();
        let bi = frequencies_of(&mut session, Kind::Word, None, 2).unwrap();
        assert!(bi.rows.iter().any(|r| r.item.to_lowercase() == "ka?a kaba" && r.count == 2), "case folded together");
        let segments: usize = crate::concordance::tests_word_counts(&mut session).iter().map(|n| n.saturating_sub(1)).sum();
        assert_eq!(bi.tokens, segments);
        assert!(bi.rows.iter().all(|r| r.item.split(' ').count() == 2 && r.count >= 2));
        assert_eq!(bi.words, words.tokens);
        // A sequence's count is the Concordance's lines for it, met in a row.
        for row in bi.rows.iter().take(20) {
            let found = crate::concordance::concordance_in(&mut session, &row.item, Kind::Word, crate::concordance::Match::Exact, None).unwrap().total;
            assert_eq!(found, row.count, "{}", row.item);
        }
        assert!(frequencies_of(&mut session, Kind::Morph, None, 2).is_err());
        assert!(frequencies_of(&mut session, Kind::Word, None, 6).is_err());
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
