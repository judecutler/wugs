//! Similar words (Scrolls): the words used in the same contexts as a word,
//! as Sketch Engine's Thesaurus finds them, but from the plain text alone,
//! with no parser: candidates for synonyms, antonyms and words of one class
//! (days, colours, verbs of motion). Distributional similarity in its
//! standard form (Levy, Goldberg and Dagan 2015): each word is a vector of
//! the words within `window` of it in the same segment, weighted by positive
//! pointwise mutual information with the context distribution smoothed
//! (counts raised to 0.75), and two words are as similar as the cosine of
//! their vectors. Words are counted under the Concordance's keys
//! (`fold_key`), each row showing the spelling met most often.
//!
//! Only words met at least `min_freq` times are compared, and only the
//! `MAX_CONTEXTS` commonest words serve as contexts, so a large corpus
//! stays within memory and seconds.

use std::collections::HashMap;

use serde::Serialize;

use crate::concordance::{each_segment, Kind, Read, Scope};
use crate::session::{project, Session};

/// Contexts are the commonest words, at most this many.
pub const MAX_CONTEXTS: usize = 3000;
/// Rows returned, most similar first.
pub const MAX_ROWS: usize = 100;
/// The widest window either side.
pub const MAX_WINDOW: usize = 5;
/// Shared contexts listed per row.
const SHARED: usize = 5;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimilarRow {
    pub item: String,
    /// Cosine of the two words' context vectors, 0–1.
    pub similarity: f64,
    pub freq: usize,
    /// The contexts that weigh most in the similarity, as spelled most often.
    pub shared: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Similar {
    pub rows: Vec<SimilarRow>,
    /// The word as spelled most often, and how often it occurs.
    pub word: String,
    pub freq: usize,
    /// Running words read, and words frequent enough to be compared.
    pub tokens: usize,
    pub compared: usize,
}

struct Words {
    keys: HashMap<String, u32>,
    spellings: Vec<HashMap<String, u32>>,
    freq: Vec<usize>,
}

impl Words {
    fn id(&mut self, key: String, spelling: String) -> u32 {
        let next = self.keys.len() as u32;
        let id = *self.keys.entry(key).or_insert(next);
        if id == next {
            self.spellings.push(HashMap::new());
            self.freq.push(0);
        }
        *self.spellings[id as usize].entry(spelling).or_default() += 1;
        self.freq[id as usize] += 1;
        id
    }
    fn spelling(&self, id: u32) -> String {
        self.spellings[id as usize].iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|s| s.0.clone()).unwrap_or_default()
    }
}

pub fn similar(session: &mut Session, word: &str, kind: Kind, window: usize, min_freq: usize, scope: &Scope) -> Result<Similar, String> {
    if !matches!(kind, Kind::Word | Kind::Translation) {
        return Err("similar words are read among words or translation words".into());
    }
    let window = window.clamp(1, MAX_WINDOW);
    let min_freq = min_freq.max(1);
    let p = project(session)?;
    let target = wugs_model::fold_key(&p.proj.nfc(word.trim()));
    let mut words = Words { keys: HashMap::new(), spellings: Vec::new(), freq: Vec::new() };
    let mut segments: Vec<Vec<u32>> = Vec::new();
    each_segment(p, Read::of(kind), scope, |seg| {
        let ids: Vec<u32> = seg
            .words
            .iter()
            .filter_map(|w| {
                let spelling = p.proj.nfc(w);
                let key = wugs_model::fold_key(&spelling);
                (!key.is_empty()).then(|| words.id(key, spelling))
            })
            .collect();
        if !ids.is_empty() {
            segments.push(ids);
        }
        Ok(())
    })?;
    let tokens: usize = segments.iter().map(Vec::len).sum();
    let Some(&node) = words.keys.get(&target) else {
        return Ok(Similar { rows: Vec::new(), word: word.trim().to_string(), freq: 0, tokens, compared: 0 });
    };
    // Context vocabulary: the commonest words, by frequency then key order.
    let mut order: Vec<u32> = (0..words.freq.len() as u32).collect();
    order.sort_by(|a, b| words.freq[*b as usize].cmp(&words.freq[*a as usize]).then(a.cmp(b)));
    let mut context_of: Vec<Option<u32>> = vec![None; words.freq.len()];
    for (i, id) in order.iter().take(MAX_CONTEXTS).enumerate() {
        context_of[*id as usize] = Some(i as u32);
    }
    let compared = |id: u32| words.freq[id as usize] >= min_freq || id == node;
    // Co-occurrence counts: target word → context → count.
    let mut rows: HashMap<u32, HashMap<u32, f64>> = HashMap::new();
    let mut col = vec![0f64; MAX_CONTEXTS.min(words.freq.len())];
    let mut total = 0f64;
    for seg in &segments {
        for (i, &w) in seg.iter().enumerate() {
            if !compared(w) {
                continue;
            }
            let from = i.saturating_sub(window);
            let to = (i + window).min(seg.len() - 1);
            for (j, &c) in seg.iter().enumerate().take(to + 1).skip(from) {
                if j == i {
                    continue;
                }
                let Some(ci) = context_of[c as usize] else { continue };
                *rows.entry(w).or_default().entry(ci).or_default() += 1.0;
                col[ci as usize] += 1.0;
                total += 1.0;
            }
        }
    }
    let node_freq = words.freq[node as usize];
    let empty = |compared| Similar { rows: Vec::new(), word: words.spelling(node), freq: node_freq, tokens, compared };
    let compared_count = rows.len();
    if total == 0.0 || !rows.contains_key(&node) {
        return Ok(empty(compared_count));
    }
    let smoothed: Vec<f64> = col.iter().map(|c| c.powf(0.75)).collect();
    let smoothed_total: f64 = smoothed.iter().sum();
    // Each row's PPMI vector, sorted by context, with its length.
    let ppmi = |counts: &HashMap<u32, f64>| -> (Vec<(u32, f64)>, f64) {
        let row_total: f64 = counts.values().sum();
        let mut v: Vec<(u32, f64)> = counts
            .iter()
            .filter_map(|(&c, &n)| {
                let pmi = ((n / total) / ((row_total / total) * (smoothed[c as usize] / smoothed_total))).log2();
                (pmi > 0.0).then_some((c, pmi))
            })
            .collect();
        v.sort_by_key(|x| x.0);
        let norm = v.iter().map(|x| x.1 * x.1).sum::<f64>().sqrt();
        (v, norm)
    };
    let (node_vec, node_norm) = ppmi(&rows[&node]);
    if node_norm == 0.0 {
        return Ok(empty(compared_count));
    }
    let node_map: HashMap<u32, f64> = node_vec.iter().copied().collect();
    // (word, similarity, the contexts weighing most)
    type Found = (u32, f64, Vec<(u32, f64)>);
    let mut out: Vec<Found> = Vec::new();
    for (&w, counts) in &rows {
        if w == node || words.freq[w as usize] < min_freq {
            continue;
        }
        let (v, norm) = ppmi(counts);
        if norm == 0.0 {
            continue;
        }
        let mut parts: Vec<(u32, f64)> = v.iter().filter_map(|(c, x)| node_map.get(c).map(|y| (*c, x * y))).collect();
        let dot: f64 = parts.iter().map(|x| x.1).sum();
        if dot <= 0.0 {
            continue;
        }
        parts.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        parts.truncate(SHARED);
        out.push((w, dot / (node_norm * norm), parts));
    }
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(words.freq[b.0 as usize].cmp(&words.freq[a.0 as usize])).then(a.0.cmp(&b.0)));
    out.truncate(MAX_ROWS);
    let rows = out
        .into_iter()
        .map(|(w, s, parts)| SimilarRow { item: words.spelling(w), similarity: s, freq: words.freq[w as usize], shared: parts.into_iter().map(|(c, _)| words.spelling(order[c as usize])).collect() })
        .collect();
    Ok(Similar { rows, word: words.spelling(node), freq: node_freq, tokens, compared: compared_count })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_in_the_same_contexts_are_similar() {
        let dir = std::env::temp_dir().join(format!("wugs-similar-test-{}", wugs_model::ulid()));
        let mut session: Session = None;
        crate::session::create_project(&mut session, dir.to_str().unwrap(), "tester").unwrap();
        // Dogs and cats bark and sleep alike; the river flows elsewhere.
        let mut lines = Vec::new();
        for _ in 0..4 {
            lines.push("the dog sleeps on the mat");
            lines.push("the cat sleeps on the mat");
            lines.push("a dog eats the food");
            lines.push("a cat eats the food");
            lines.push("the river flows to the sea");
        }
        let text = crate::session::create_text(&mut session, "Pets").unwrap();
        for (i, line) in lines.iter().enumerate() {
            crate::session::add_segment(&mut session, &text, None, Some(i as f64 * 1000.0), Some(i as f64 * 1000.0 + 900.0), line).unwrap();
        }
        let r = similar(&mut session, "Dog", Kind::Word, 2, 3, &Scope::all()).unwrap();
        assert_eq!((r.word.as_str(), r.freq, r.tokens), ("dog", 8, 4 * (6 + 6 + 5 + 5 + 6)));
        assert_eq!(r.rows[0].item, "cat", "{:?}", r.rows);
        assert!(r.rows[0].similarity > 0.9, "{:?}", r.rows[0]);
        assert!(r.rows[0].shared.iter().any(|s| s == "sleeps" || s == "eats"), "{:?}", r.rows[0].shared);
        let river = r.rows.iter().find(|x| x.item == "river").map(|x| x.similarity).unwrap_or(0.0);
        assert!(river < r.rows[0].similarity / 2.0, "river {river}");
        // A word not in the corpus gives nothing; a high minimum leaves rare words out.
        assert!(similar(&mut session, "horse", Kind::Word, 2, 3, &Scope::all()).unwrap().rows.is_empty());
        assert!(similar(&mut session, "dog", Kind::Word, 2, 100, &Scope::all()).unwrap().rows.is_empty());
        assert!(similar(&mut session, "dog", Kind::Morph, 2, 3, &Scope::all()).is_err());
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
