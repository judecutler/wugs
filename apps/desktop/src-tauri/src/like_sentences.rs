//! Sentences like this one: while glossing or translating a sentence, the
//! sentences of the project that share most of its words (weighted by how
//! rare each word is), with their translations, so a phrase translated
//! before is translated the same way again and a word glossed before is
//! seen in use. A translation memory of the project's own texts.
//!
//! Every transcription segment's words come from the corpus index once per
//! change to the project and are kept as word ids with each word's inverse
//! document frequency; a question scores only the segments that share a word
//! with it, by the cosine of their binary tf-idf vectors.

use std::collections::HashMap;
use std::rc::Rc;

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{project, Session};

/// Under this score a sentence is not offered.
pub const MIN_SCORE: f64 = 0.2;
/// At most this many sentences are offered.
pub const MAX_LIKE: usize = 5;

struct Seg {
    id: String,
    text: String,
    title: String,
    tier: String,
    words: Vec<u32>,
    norm: f64,
}

struct Index {
    segs: Vec<Seg>,
    at: HashMap<String, usize>,
    postings: Vec<Vec<u32>>,
    idf: Vec<f64>,
    spelling: Vec<String>,
}

thread_local! {
    /// Kept while nothing has been written: keyed by project folder and newest op.
    static INDEX: std::cell::RefCell<Option<(std::path::PathBuf, Option<String>, Rc<Index>)>> = const { std::cell::RefCell::new(None) };
}

fn index(p: &wugs_core::Project) -> Result<Rc<Index>, String> {
    let newest = p.proj.max_hlc()?;
    if let Some(ix) = INDEX.with(|c| c.borrow().as_ref().filter(|(root, hlc, _)| *root == p.root && *hlc == newest).map(|(_, _, ix)| ix.clone())) {
        return Ok(ix);
    }
    let mut ids: HashMap<String, u32> = HashMap::new();
    let mut spelling: Vec<String> = Vec::new();
    let mut segs = Vec::new();
    each_segment(p, Read::Words, &Scope::all(), |seg| {
        let mut words: Vec<u32> = Vec::new();
        for w in &seg.words {
            let key = wugs_model::fold_key(&p.proj.nfc(w));
            if key.is_empty() {
                continue;
            }
            let next = ids.len() as u32;
            let id = *ids.entry(key).or_insert_with(|| {
                spelling.push(w.clone());
                next
            });
            words.push(id);
        }
        words.sort_unstable();
        words.dedup();
        if !words.is_empty() {
            segs.push(Seg { id: seg.id.clone(), text: seg.text.clone(), title: seg.title.clone(), tier: seg.tier.clone(), words, norm: 0.0 });
        }
        Ok(())
    })?;
    let mut postings: Vec<Vec<u32>> = vec![Vec::new(); ids.len()];
    for (i, s) in segs.iter().enumerate() {
        for &w in &s.words {
            postings[w as usize].push(i as u32);
        }
    }
    let n = segs.len().max(1) as f64;
    let idf: Vec<f64> = postings.iter().map(|p| (n / p.len().max(1) as f64).ln()).collect();
    for s in &mut segs {
        s.norm = s.words.iter().map(|&w| idf[w as usize].powi(2)).sum::<f64>().sqrt();
    }
    let at = segs.iter().enumerate().map(|(i, s)| (s.id.clone(), i)).collect();
    let ix = Rc::new(Index { segs, at, postings, idf, spelling });
    INDEX.with(|c| *c.borrow_mut() = Some((p.root.clone(), newest, ix.clone())));
    Ok(ix)
}

/// A sentence like the one asked about.
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Like {
    pub text_id: String,
    pub text_title: String,
    pub tier_id: String,
    pub segment_id: String,
    pub content: String,
    /// Its first translation, when it has one.
    pub translation: Option<String>,
    /// The words both share, rarest first (at most six; a word every sentence has is left out).
    pub shared: Vec<String>,
    /// Cosine of the two sentences' tf-idf vectors, 0 to 1.
    pub score: f64,
}

/// The sentences most like `segment` (not itself), best first, translated
/// ones first among equals; none when it has no words or the project too few
/// sentences to tell rare words from common ones.
pub fn like(session: &mut Session, segment: &str) -> Result<Vec<Like>, String> {
    let p = project(session)?;
    let ix = index(p)?;
    let Some(&me) = ix.at.get(segment) else { return Ok(Vec::new()) };
    let mine = &ix.segs[me];
    if mine.norm == 0.0 {
        return Ok(Vec::new());
    }
    let mut dot: HashMap<u32, f64> = HashMap::new();
    for &w in &mine.words {
        let weight = ix.idf[w as usize].powi(2);
        if weight == 0.0 {
            continue;
        }
        for &other in &ix.postings[w as usize] {
            if other as usize != me {
                *dot.entry(other).or_default() += weight;
            }
        }
    }
    let mut scored: Vec<(u32, f64)> = dot.into_iter().filter_map(|(o, d)| {
        let norm = ix.segs[o as usize].norm;
        (norm > 0.0).then(|| (o, d / (mine.norm * norm)))
    }).filter(|(_, s)| *s >= MIN_SCORE).collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut out = Vec::new();
    for (o, score) in scored.into_iter().take(MAX_LIKE * 3) {
        let s = &ix.segs[o as usize];
        let content = p.proj.field_str(&s.id, "content")?.unwrap_or_default();
        let translation = crate::interchange::translations_of(p, &s.id)?.into_iter().next().map(|(_, t)| t);
        let mut shared: Vec<u32> = s.words.iter().copied().filter(|w| ix.idf[*w as usize] > 0.0 && mine.words.binary_search(w).is_ok()).collect();
        shared.sort_by(|a, b| ix.idf[*b as usize].total_cmp(&ix.idf[*a as usize]));
        out.push(Like {
            text_id: s.text.clone(),
            text_title: s.title.clone(),
            tier_id: s.tier.clone(),
            segment_id: s.id.clone(),
            content,
            translation,
            shared: shared.into_iter().take(6).map(|w| ix.spelling[w as usize].clone()).collect(),
            score: (score * 1000.0).round() / 1000.0,
        });
    }
    // Among sentences within a hundredth of each other, a translated one first: it is what helps.
    out.sort_by(|a, b| ((b.score * 100.0).round()).total_cmp(&(a.score * 100.0).round()).then(b.translation.is_some().cmp(&a.translation.is_some())));
    out.truncate(MAX_LIKE);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session;

    #[test]
    fn the_sentence_sharing_the_rare_words_comes_first() {
        let dir = std::env::temp_dir().join(format!("wugs-like-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, dir.to_str().unwrap(), "tester").unwrap();
        let text = session::create_text(&mut s, "Market").unwrap();
        let lines = ["ka nimo pela sato", "ka nimo pela rua", "ka toru miri", "ka sela wuno", "ka tepi lomo", "Ka nimo pela sato!"];
        let mut ids = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            ids.push(session::add_segment(&mut s, &text, None, Some(i as f64 * 1000.0), Some(i as f64 * 1000.0 + 900.0), line).unwrap().id);
        }
        let found = like(&mut s, &ids[0]).unwrap();
        let order: Vec<&str> = found.iter().map(|l| l.content.as_str()).collect();
        assert_eq!(order, ["Ka nimo pela sato!", "ka nimo pela rua"], "the same words first, case and punctuation aside; «ka» alone counts for nothing");
        assert!((found[0].score - 1.0).abs() < 1e-9);
        assert_eq!(found[1].shared, vec!["nimo".to_string(), "pela".to_string()]);
        assert!(found.iter().all(|l| l.segment_id != ids[0]), "never itself");
        assert!(like(&mut s, &ids[2]).unwrap().is_empty(), "nothing shares a rare word with «ka toru miri»");
        assert!(like(&mut s, "no-such-segment").unwrap().is_empty());
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
