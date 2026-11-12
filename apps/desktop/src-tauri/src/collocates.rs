//! Collocates (Scrolls, SCROLLS.md §5 step 3): which words keep company with
//! a word, the way AntConc's Collocates tool and Sketch Engine's collocation
//! lists measure it. For every occurrence of the node (a word or a `*`/`?`
//! pattern, met as the Concordance meets it) the words up to `left` before
//! and `right` after it in the same segment are counted; a window never
//! crosses a segment, since a segment is an utterance or sentence.
//!
//! Each collocate gets two association scores over the 2×2 table of "in a
//! node's window or not" against "this word or not":
//! - **Log-likelihood** (Dunning 1993), the default sort in AntConc 4: how
//!   surprising the co-occurrence count is. Negative when the pair occurs
//!   less often than chance would put it together.
//! - **Mutual information** (Church and Hanks 1990): log2 of observed over
//!   expected; high for rare pairs, so a minimum co-occurrence count goes
//!   with it.
//!
//! The expected count is `window tokens × f(collocate) / N`, where the window
//! tokens are every word slot the node's windows cover and N every word in
//! the scope. Words are counted under the Concordance's keys (`fold_key`),
//! each row showing the spelling met most often.

use std::collections::HashMap;

use serde::Serialize;

use crate::concordance::{each_segment, glob, Kind};
use crate::session::{project, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollocateRow {
    pub item: String,
    /// Times it occurs in a node's window.
    pub together: usize,
    pub left: usize,
    pub right: usize,
    /// Times it occurs in the whole scope.
    pub freq: usize,
    pub log_likelihood: f64,
    pub mutual_information: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collocates {
    /// Strongest by log-likelihood first.
    pub rows: Vec<CollocateRow>,
    /// Occurrences of the node.
    pub node: usize,
    /// Words in the scope.
    pub tokens: usize,
    /// Collocates found, even past the limit and the minimum.
    pub types: usize,
}

/// At most this many rows are returned.
pub const LIMIT: usize = 2000;
/// The widest window AntConc offers on a side.
pub const MAX_SPAN: usize = 10;

#[derive(Default)]
struct Tally {
    together: usize,
    left: usize,
    right: usize,
    spellings: Vec<(String, usize)>,
}

#[allow(clippy::too_many_arguments)]
pub fn collocates(session: &mut Session, node: &str, kind: Kind, left: usize, right: usize, min_together: usize, text: Option<&str>) -> Result<Collocates, String> {
    if !matches!(kind, Kind::Word | Kind::Translation) {
        return Err("collocates are counted between words or translation words".into());
    }
    if left == 0 && right == 0 {
        return Err("the window needs at least one word on one side".into());
    }
    let (left, right) = (left.min(MAX_SPAN), right.min(MAX_SPAN));
    let p = project(session)?;
    let pattern: Vec<char> = wugs_model::fold_key(&p.proj.nfc(node.trim())).chars().collect();
    let empty = Collocates { rows: Vec::new(), node: 0, tokens: 0, types: 0 };
    if pattern.is_empty() || pattern.iter().all(|c| *c == '*') {
        return Ok(empty);
    }
    let mut freq: HashMap<String, usize> = HashMap::new();
    let mut near: HashMap<String, Tally> = HashMap::new();
    let (mut tokens, mut nodes, mut slots) = (0usize, 0usize, 0usize);
    each_segment(p, kind == Kind::Translation, text, |seg| {
        let words: Vec<(String, String)> = seg
            .words
            .iter()
            .map(|w| {
                let spelling = p.proj.nfc(w);
                (wugs_model::fold_key(&spelling), spelling)
            })
            .filter(|(k, _)| !k.is_empty())
            .collect();
        tokens += words.len();
        for (k, _) in &words {
            *freq.entry(k.clone()).or_default() += 1;
        }
        for (i, (k, _)) in words.iter().enumerate() {
            if !glob(&pattern, &k.chars().collect::<Vec<_>>()) {
                continue;
            }
            nodes += 1;
            let from = i.saturating_sub(left);
            let to = (i + right).min(words.len().saturating_sub(1));
            for (j, (ck, spelling)) in words.iter().enumerate().take(to + 1).skip(from) {
                if j == i {
                    continue;
                }
                slots += 1;
                let t = near.entry(ck.clone()).or_default();
                t.together += 1;
                if j < i {
                    t.left += 1;
                } else {
                    t.right += 1;
                }
                match t.spellings.iter_mut().find(|(s, _)| s == spelling) {
                    Some((_, n)) => *n += 1,
                    None => t.spellings.push((spelling.clone(), 1)),
                }
            }
        }
        Ok(())
    })?;
    let types = near.len();
    let mut rows: Vec<CollocateRow> = near
        .into_iter()
        .filter(|(_, t)| t.together >= min_together.max(1))
        .map(|(key, t)| {
            let f = freq.get(&key).copied().unwrap_or(t.together);
            let (ll, mi) = scores(t.together, slots, f, tokens);
            let item = t.spellings.iter().fold(None::<&(String, usize)>, |best, s| if best.is_none_or(|b| s.1 > b.1) { Some(s) } else { best }).map(|s| s.0.clone()).unwrap_or(key);
            CollocateRow { item, together: t.together, left: t.left, right: t.right, freq: f, log_likelihood: ll, mutual_information: mi }
        })
        .collect();
    rows.sort_by(|a, b| b.log_likelihood.total_cmp(&a.log_likelihood).then(b.together.cmp(&a.together)).then(a.item.cmp(&b.item)));
    rows.truncate(LIMIT);
    Ok(Collocates { rows, node: nodes, tokens, types })
}

/// Log-likelihood (signed: negative when the pair is rarer than chance) and
/// mutual information for `o11` co-occurrences, `window` slots in the node's
/// windows, a collocate frequency `f` and `n` words in all.
pub fn scores(o11: usize, window: usize, f: usize, n: usize) -> (f64, f64) {
    let (o11, r1, c1, n) = (o11 as f64, window as f64, f as f64, n as f64);
    if n == 0.0 || r1 == 0.0 || c1 == 0.0 {
        return (0.0, 0.0);
    }
    // The other three cells; the window can count a word more than once
    // (overlapping windows), so cells are kept at zero or above.
    let o12 = (r1 - o11).max(0.0);
    let o21 = (c1 - o11).max(0.0);
    let o22 = (n - r1 - o21).max(0.0);
    let r2 = o21 + o22;
    let c2 = o12 + o22;
    let total = o11 + o12 + o21 + o22;
    let e = |r: f64, c: f64| r * c / total;
    let term = |o: f64, e: f64| if o > 0.0 && e > 0.0 { o * (o / e).ln() } else { 0.0 };
    let (e11, e12, e21, e22) = (e(r1, c1), e(r1, c2), e(r2, c1), e(r2, c2));
    let ll = 2.0 * (term(o11, e11) + term(o12, e12) + term(o21, e21) + term(o22, e22));
    let mi = if o11 > 0.0 && e11 > 0.0 { (o11 / e11).log2() } else { 0.0 };
    (if o11 < e11 { -ll } else { ll }, mi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_match_published_worked_values() {
        // A word pair met 10 times, in windows covering 100 slots, the
        // collocate 20 times among 10,000 words: expected 0.2.
        let (ll, mi) = scores(10, 100, 20, 10_000);
        assert!((mi - (10.0f64 / 0.2).log2()).abs() < 1e-9, "{mi}");
        // Dunning's G² for that table, worked by hand:
        // 2·[10 ln(10/0.2) + 90 ln(90/99.8) + 10 ln(10/19.8) + 9890 ln(9890/9880.2)].
        let hand = 2.0 * (10.0 * (10.0f64 / 0.2).ln() + 90.0 * (90.0f64 / 99.8).ln() + 10.0 * (10.0f64 / 19.8).ln() + 9890.0 * (9890.0f64 / 9880.2).ln());
        assert!((ll - hand).abs() < 1e-6, "{ll} vs {hand}");
        // Rarer than chance: negative.
        assert!(scores(1, 1000, 20, 10_000).0 < 0.0);
        assert_eq!(scores(0, 0, 0, 0), (0.0, 0.0));
    }

    #[test]
    fn counts_neighbours_within_a_segment() {
        let dir = std::env::temp_dir().join(format!("wugs-colloc-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        let text = crate::session::list_texts(&mut session).unwrap()[0].id.clone();
        for (i, line) in ["the old fox drank", "the old fox ran", "a young fox slept", "Old Fox."].iter().enumerate() {
            let at = 30_000.0 + i as f64 * 1000.0;
            crate::session::add_segment(&mut session, &text, None, Some(at), Some(at + 900.0), line).unwrap();
        }
        let c = collocates(&mut session, "fox", Kind::Word, 1, 1, 1, None).unwrap();
        assert_eq!(c.node, 4);
        let old = c.rows.iter().find(|r| r.item.eq_ignore_ascii_case("old")).expect("old");
        assert_eq!((old.together, old.left, old.right), (3, 3, 0), "case folded, left only");
        assert_eq!(old.freq, 3);
        assert_eq!(c.rows[0].item.to_lowercase(), "old", "the strongest first");
        // A window never reaches into the next segment: «drank» never meets «the».
        let two = collocates(&mut session, "drank", Kind::Word, 5, 5, 1, None).unwrap();
        assert!(two.rows.iter().all(|r| r.item != "ran" && r.item != "a"), "{:?}", two.rows.iter().map(|r| &r.item).collect::<Vec<_>>());
        // The minimum hides one-offs; patterns work.
        assert!(collocates(&mut session, "fox", Kind::Word, 1, 1, 2, None).unwrap().rows.iter().all(|r| r.together >= 2));
        assert_eq!(collocates(&mut session, "f?x", Kind::Word, 1, 1, 1, None).unwrap().node, 4);
        assert_eq!(collocates(&mut session, "*", Kind::Word, 1, 1, 1, None).unwrap().node, 0);
        assert!(collocates(&mut session, "fox", Kind::Word, 0, 0, 1, None).is_err());
        assert!(collocates(&mut session, "fox", Kind::Gloss, 1, 1, 1, None).is_err());
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
