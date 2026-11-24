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

use crate::concordance::{Read, Scope, bare, each_segment, morphs_of, Kind};
use crate::session::{project, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyRow {
    /// The spelling met most often.
    pub item: String,
    pub count: usize,
    /// How many texts it occurs in.
    pub texts: usize,
    /// For single words of transcriptions: `entry` when an entry or one of
    /// its forms (allomorph, variant, inflected form) is spelled this way,
    /// `analysed` when not but some use of it is broken into morphs, else
    /// `none`: the words a dictionary still lacks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lexicon: Option<&'static str>,
    /// How evenly it is spread over the texts: Gries's DP normalised
    /// (`dispersion::dp`), 0 in step with each text's share of the items
    /// counted, 1 all in one text. None with fewer than two texts.
    pub dp: Option<f64>,
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
    /// Words with no lemma or tag yet (lemma and tag lists only).
    pub unlabelled: usize,
}

/// At most this many rows are returned; `types` says how many there were.
pub const LIMIT: usize = 5000;

#[derive(Default)]
struct Tally {
    first: usize,
    count: usize,
    spellings: Vec<(String, usize)>,
    /// (text index, occurrences there), in the order the texts were met.
    texts: Vec<(usize, usize)>,
}

pub fn frequencies(session: &mut Session, kind: Kind, scope: &Scope) -> Result<Frequencies, String> {
    frequencies_of(session, kind, scope, 1)
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
pub fn frequencies_of(session: &mut Session, kind: Kind, scope: &Scope, size: usize) -> Result<Frequencies, String> {
    if kind == Kind::Sense {
        return Err("a frequency list counts words, morphs, glosses, translation words, lemmas or tags".into());
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
    let mut unlabelled = 0usize;
    // Each text met, with how many items were counted in it (the parts DP compares).
    let mut text_index: HashMap<String, usize> = HashMap::new();
    let mut sizes: Vec<usize> = Vec::new();
    let mut count = |key: String, spelling: &str, text: &str, tokens: &mut usize| {
        if key.is_empty() {
            return;
        }
        *tokens += 1;
        let at = *text_index.entry(text.to_string()).or_insert_with(|| {
            sizes.push(0);
            sizes.len() - 1
        });
        sizes[at] += 1;
        let next = tallies.len();
        let t = tallies.entry(key).or_insert_with(|| Tally { first: next, ..Tally::default() });
        t.count += 1;
        match t.spellings.iter_mut().find(|(s, _)| s == spelling) {
            Some((_, n)) => *n += 1,
            None => t.spellings.push((spelling.to_string(), 1)),
        }
        match t.texts.iter_mut().rev().find(|(i, _)| *i == at) {
            Some((_, n)) => *n += 1,
            None => t.texts.push((at, 1)),
        }
    };
    each_segment(p, Read::of(kind), scope, |seg| {
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
            Kind::Lemma | Kind::Tag => {
                // Lemmas and tags as a CQP query reads them (CoNLL-U analysis,
                // else a glossed word's root entry), written exactly, so a row's
                // count is the lines `[lemma="…"]` finds; punctuation is not a word.
                for t in crate::cqp::tokens_of(p, &seg.id)? {
                    if t.upos == "PUNCT" {
                        continue;
                    }
                    running += 1;
                    let value = p.proj.nfc(if kind == Kind::Lemma { &t.lemma } else { &t.upos });
                    if value.is_empty() {
                        unlabelled += 1;
                    } else {
                        count(value.clone(), &value, &seg.text, &mut tokens);
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
    let mut rows: Vec<FrequencyRow> = all
        .into_iter()
        .map(|t| {
            let item = t.spellings.iter().fold(None::<&(String, usize)>, |best, s| if best.is_none_or(|b| s.1 > b.1) { Some(s) } else { best }).map(|s| s.0.clone()).unwrap_or_default();
            let mut hits = vec![0usize; sizes.len()];
            for (i, n) in &t.texts {
                hits[*i] = *n;
            }
            let dp = crate::dispersion::dp(&sizes, &hits).1;
            FrequencyRow { item, count: t.count, texts: t.texts.len(), lexicon: None, dp }
        })
        .collect();
    if kind == Kind::Word && size == 1 {
        mark_lexicon(p, &mut rows)?;
    }
    let words = if matches!(kind, Kind::Word | Kind::Translation | Kind::Lemma | Kind::Tag) { running } else { tokens };
    Ok(Frequencies { rows, tokens, types, words, once, unlabelled })
}

/// Say of each word whether the lexicon has it (`FrequencyRow::lexicon`).
/// Analysed uses are found in one query over every row's key.
fn mark_lexicon(p: &wugs_core::Project, rows: &mut [FrequencyRow]) -> Result<(), String> {
    let keys: Vec<String> = rows.iter().map(|r| wugs_model::fold_key(&r.item)).collect();
    let analysed: std::collections::HashSet<String> = p.proj.tokens_by_keys(&keys)?.into_iter().filter(|t| t.3).map(|t| t.0).collect();
    for (row, key) in rows.iter_mut().zip(&keys) {
        row.lexicon = Some(if crate::consistency::known(p, &wugs_model::lexicon_key(&row.item))? {
            "entry"
        } else if analysed.contains(key) {
            "analysed"
        } else {
            "none"
        });
    }
    Ok(())
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
        crate::sample::add_morphology(&mut session).unwrap();
        // A segment with a glottal stop written `?` and a reduplicated word
        // whose two morphs share a gloss: the cases a count can get wrong.
        let text = crate::session::list_texts(&mut session).unwrap()[0].id.clone();
        let seg = crate::session::add_segment(&mut session, &text, None, Some(19_000.0), Some(19_900.0), "ka?a kaba lolo").unwrap().id;
        let forms: Vec<String> = ["ka?a", "kaba", "lolo"].iter().map(|s| s.to_string()).collect();
        let view = crate::gloss::gloss_tokenize(&mut session, &text, &seg, &forms).unwrap().view;
        let m = |surface: &str, kind: &str| crate::gloss::MorphSpecView { surface: surface.into(), morph_type: kind.into(), gloss: Some("PL".into()), glosses_by_lang: Default::default(), sense: None };
        crate::gloss::gloss_apply(&mut session, &text, &seg, &view.words[2].id, vec![m("lo", "reduplication"), m("lo", "root")]).unwrap();
        let words = frequencies(&mut session, Kind::Word, &Scope::all()).unwrap();
        assert!(words.tokens > 0 && words.types <= words.tokens);
        // Every row's count is what the Concordance finds for that exact item,
        // in the whole project and in the one text.
        for kind in [Kind::Word, Kind::Morph, Kind::Gloss, Kind::Translation] {
            for scope in [Scope::all(), Scope::text(&text)] {
                for row in frequencies(&mut session, kind, &scope).unwrap().rows {
                    let found = crate::concordance::concordance_in(&mut session, &row.item, kind, crate::concordance::Match::Exact, &scope).unwrap().total;
                    assert_eq!(found, row.count, "{kind:?} {}", row.item);
                }
            }
        }
        // Give every entry a part of speech, so glossed words carry a tag.
        {
            let p = crate::session::project(&mut session).unwrap();
            for lex in p.proj.entities_of_kind("lexeme").unwrap() {
                p.set_lexeme_field(&lex, "pos-of-speech", "num").unwrap();
            }
        }
        // Lemma and tag rows equal the lines a CQP query finds for them.
        for (kind, attr) in [(Kind::Lemma, "lemma"), (Kind::Tag, "upos")] {
            let f = frequencies(&mut session, kind, &Scope::all()).unwrap();
            assert!(!f.rows.is_empty(), "{attr}: the sample's glossed words have lemmas and tags");
            assert_eq!(f.words, f.tokens + f.unlabelled);
            for row in f.rows {
                let q = format!("[{attr}=\"{}\"]", crate::cqp::literal(&row.item));
                assert_eq!(crate::cqp::run(&mut session, &q, None).unwrap().total, row.count, "{q}");
            }
        }
        let lo = frequencies(&mut session, Kind::Morph, &Scope::all()).unwrap().rows.into_iter().find(|r| r.item == "lo").unwrap();
        assert_eq!(lo.count, 1, "a word with lo twice counts once");
        assert!(words.rows.windows(2).all(|w| w[0].count >= w[1].count), "most frequent first");
        let morphs = frequencies(&mut session, Kind::Morph, &Scope::all()).unwrap();
        let kani = morphs.rows.iter().find(|r| r.item == "kani").expect("kani counted");
        assert_eq!(kani.count, 1);
        let ni = morphs.rows.iter().find(|r| r.item == "ni").expect("markers dropped");
        assert_eq!(ni.texts, 1);
        assert!(frequencies(&mut session, Kind::Gloss, &Scope::all()).unwrap().rows.iter().any(|r| r.item == "1SG"));
        let t = frequencies(&mut session, Kind::Translation, &Scope::all()).unwrap();
        assert!(t.rows.iter().any(|r| r.item.eq_ignore_ascii_case("three")));
        // One text: the sample has one, so it is the whole project.
        assert_eq!(frequencies(&mut session, Kind::Word, &Scope::text(&text)).unwrap().tokens, words.tokens);
        assert_eq!(frequencies(&mut session, Kind::Word, &Scope::text("txt_none")).unwrap().tokens, 0);
        // Texts picked by a detail: the one text with it, then none.
        crate::metadata::set_text_metadata(&mut session, &text, "Genre", "counting").unwrap();
        let by = |value: &str| Scope::of(None, Some(crate::concordance::Where { key: "genre".into(), value: value.into() }));
        assert_eq!(frequencies(&mut session, Kind::Word, &by("counting")).unwrap().tokens, words.tokens);
        assert_eq!(frequencies(&mut session, Kind::Word, &by("news")).unwrap().tokens, 0);
        assert_eq!(crate::concordance::concordance_in(&mut session, "wʌn", Kind::Word, crate::concordance::Match::Exact, &by("news")).unwrap().total, 0);
        assert!(frequencies(&mut session, Kind::Sense, &Scope::all()).is_err());
        // Bigrams: one per pair of words in a row, never across segments.
        crate::session::add_segment(&mut session, &text, None, Some(20_000.0), Some(20_900.0), "Ka?a kaba").unwrap();
        let words = frequencies(&mut session, Kind::Word, &Scope::all()).unwrap();
        let bi = frequencies_of(&mut session, Kind::Word, &Scope::all(), 2).unwrap();
        assert!(bi.rows.iter().any(|r| r.item.to_lowercase() == "ka?a kaba" && r.count == 2), "case folded together");
        let segments: usize = crate::concordance::tests_word_counts(&mut session).iter().map(|n| n.saturating_sub(1)).sum();
        assert_eq!(bi.tokens, segments);
        assert!(bi.rows.iter().all(|r| r.item.split(' ').count() == 2 && r.count >= 2));
        assert_eq!(bi.words, words.tokens);
        // A sequence's count is the Concordance's lines for it, met in a row.
        for row in bi.rows.iter().take(20) {
            let found = crate::concordance::concordance_in(&mut session, &row.item, Kind::Word, crate::concordance::Match::Exact, &Scope::all()).unwrap().total;
            assert_eq!(found, row.count, "{}", row.item);
        }
        assert!(frequencies_of(&mut session, Kind::Morph, &Scope::all(), 2).is_err());
        assert!(frequencies_of(&mut session, Kind::Word, &Scope::all(), 6).is_err());
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rows_say_how_evenly_they_spread() {
        let dir = std::env::temp_dir().join(format!("wugs-freq-dp-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        let only = |session: &mut Session| frequencies(session, Kind::Word, &Scope::all()).unwrap();
        assert!(only(&mut session).rows.iter().all(|r| r.dp.is_none()), "one text: nothing to compare");
        let a = crate::session::create_text(&mut session, "A").unwrap();
        let b = crate::session::create_text(&mut session, "B").unwrap();
        crate::session::add_segment(&mut session, &a, None, None, None, "bo ka bo ka").unwrap();
        crate::session::add_segment(&mut session, &b, None, None, None, "bo zu zu zu").unwrap();
        let f = only(&mut session);
        let dp = |word: &str| f.rows.iter().find(|r| r.item == word).unwrap().dp.unwrap();
        // Against the parts' sizes: every text's words, the sample's included.
        let by: Vec<(String, usize)> = {
            let mut by: Vec<(String, usize)> = Vec::new();
            crate::concordance::each_segment(crate::session::project(&mut session).unwrap(), crate::concordance::Read::Words, &Scope::all(), |seg| {
                match by.iter_mut().find(|(t, _)| *t == seg.text) {
                    Some((_, n)) => *n += seg.words.len(),
                    None => by.push((seg.text.clone(), seg.words.len())),
                }
                Ok(())
            })
            .unwrap();
            by
        };
        let sizes: Vec<usize> = by.iter().map(|(_, n)| *n).collect();
        assert_eq!(sizes.iter().sum::<usize>(), f.words);
        let zu: Vec<usize> = by.iter().map(|(t, _)| if *t == b { 3 } else { 0 }).collect();
        assert!((dp("zu") - crate::dispersion::dp(&sizes, &zu).1.unwrap()).abs() < 1e-12, "Gries's DP, normalised");
        assert!(dp("zu") > dp("bo"), "zu sits in one text, bo in two");
        assert!(dp("bo") < 1.0 && dp("zu") > 0.5);
        // One text picked: no spread to measure.
        assert!(frequencies(&mut session, Kind::Word, &Scope::text(&a)).unwrap().rows.iter().all(|r| r.dp.is_none()));
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn words_say_whether_the_lexicon_has_them() {
        let dir = std::env::temp_dir().join(format!("wugs-freq-lexicon-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        let text = crate::session::list_texts(&mut session).unwrap()[0].id.clone();
        crate::session::add_segment(&mut session, &text, None, Some(20_000.0), Some(20_900.0), "Blorp blorps zibu").unwrap();
        let seg = crate::session::add_segment(&mut session, &text, None, Some(21_000.0), Some(21_900.0), "zibu").unwrap().id;
        let forms = vec!["zibu".to_string()];
        let view = crate::gloss::gloss_tokenize(&mut session, &text, &seg, &forms).unwrap().view;
        let m = |surface: &str, kind: &str, gloss: &str| crate::gloss::MorphSpecView { surface: surface.into(), morph_type: kind.into(), gloss: Some(gloss.into()), glosses_by_lang: Default::default(), sense: None };
        crate::gloss::gloss_apply(&mut session, &text, &seg, &view.words[0].id, vec![m("zib", "root", "eat"), m("u", "suffix", "PST")]).unwrap();
        let status = |session: &mut Session, word: &str| {
            let list = frequencies(session, Kind::Word, &Scope::all()).unwrap();
            list.rows.into_iter().find(|r| r.item.to_lowercase() == word).and_then(|r| r.lexicon).unwrap()
        };
        assert_eq!(status(&mut session, "blorp"), "none");
        assert_eq!(status(&mut session, "zibu"), "analysed", "a word broken into morphs somewhere");
        let id = crate::lexicon::lexicon_add(&mut session, "blorp", "thing").unwrap().entry.unwrap().id;
        assert_eq!(status(&mut session, "blorp"), "entry", "a headword, case folded");
        assert_eq!(status(&mut session, "blorps"), "none");
        crate::lexicon::lexicon_add_allomorph(&mut session, &id, "blorps").unwrap();
        assert_eq!(status(&mut session, "blorps"), "entry", "one of an entry's forms");
        assert!(frequencies(&mut session, Kind::Morph, &Scope::all()).unwrap().rows.iter().all(|r| r.lexicon.is_none()), "only words are marked");
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
