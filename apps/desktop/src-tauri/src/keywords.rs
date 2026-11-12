//! Keywords (Scrolls, SCROLLS.md §5 step 6, started early): which words one
//! text uses more, or less, than the rest of the project, the way AntConc's
//! Keyword List and Sketch Engine's keywords compare a focus corpus with a
//! reference corpus. The focus is one text; the reference is every other
//! text in the project (a reference corpus from a dataset package comes with
//! the index, row 37).
//!
//! Words are counted under the Concordance's keys (`fold_key`), each row
//! showing the spelling met most often. For each word met in either corpus:
//! - **Log-likelihood** (Rayson and Garside 2000), signed: positive when the
//!   text uses the word more than the reference, negative when less.
//! - **Log ratio** (Hardie 2014): log2 of the ratio of relative frequencies,
//!   an effect size (1 = twice as often). A zero count is taken as 0.5 so
//!   words missing from one side get a finite value.
//! - **%DIFF** (Gabrielatos and Marchi 2012): the difference of relative
//!   frequencies as a percentage of the reference's, with the same 0.5.

use std::collections::HashMap;

use serde::Serialize;

use crate::concordance::{each_segment, Kind};
use crate::session::{project, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeywordRow {
    pub item: String,
    /// Times in the text.
    pub focus: usize,
    /// Times in the rest of the project.
    pub reference: usize,
    pub log_likelihood: f64,
    pub log_ratio: f64,
    pub percent_diff: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Keywords {
    /// Most key first (positive), least key last (negative).
    pub rows: Vec<KeywordRow>,
    /// Words in the text and in the rest.
    pub focus_tokens: usize,
    pub reference_tokens: usize,
    /// Words compared, even past the limit and the minimum.
    pub types: usize,
}

pub const LIMIT: usize = 2000;

#[derive(Default)]
struct Tally {
    focus: usize,
    reference: usize,
    spellings: Vec<(String, usize)>,
}

pub fn keywords(session: &mut Session, kind: Kind, text: &str, min_focus: usize) -> Result<Keywords, String> {
    if !matches!(kind, Kind::Word | Kind::Translation) {
        return Err("keywords are counted between words or translation words".into());
    }
    let p = project(session)?;
    let mut tallies: HashMap<String, Tally> = HashMap::new();
    let (mut c, mut d) = (0usize, 0usize);
    each_segment(p, kind == Kind::Translation, None, |seg| {
        let here = seg.text == text;
        for w in &seg.words {
            let spelling = p.proj.nfc(w);
            let key = wugs_model::fold_key(&spelling);
            if key.is_empty() {
                continue;
            }
            let t = tallies.entry(key).or_default();
            if here {
                c += 1;
                t.focus += 1;
            } else {
                d += 1;
                t.reference += 1;
            }
            match t.spellings.iter_mut().find(|(s, _)| *s == spelling) {
                Some((_, n)) => *n += 1,
                None => t.spellings.push((spelling, 1)),
            }
        }
        Ok(())
    })?;
    let types = tallies.len();
    let mut rows: Vec<KeywordRow> = tallies
        .into_iter()
        .filter(|(_, t)| t.focus >= min_focus.max(1) || (t.focus == 0 && t.reference >= min_focus.max(1)))
        .map(|(key, t)| {
            let (ll, lr, pd) = scores(t.focus, t.reference, c, d);
            let item = t.spellings.iter().fold(None::<&(String, usize)>, |best, s| if best.is_none_or(|b| s.1 > b.1) { Some(s) } else { best }).map(|s| s.0.clone()).unwrap_or(key);
            KeywordRow { item, focus: t.focus, reference: t.reference, log_likelihood: ll, log_ratio: lr, percent_diff: pd }
        })
        .collect();
    rows.sort_by(|a, b| b.log_likelihood.total_cmp(&a.log_likelihood).then(a.item.cmp(&b.item)));
    if rows.len() > LIMIT {
        // Keep the most key at both ends.
        let tail = rows.split_off(rows.len() - LIMIT / 4);
        rows.truncate(LIMIT - tail.len());
        rows.extend(tail);
    }
    Ok(Keywords { rows, focus_tokens: c, reference_tokens: d, types })
}

/// Signed log-likelihood, log ratio and %DIFF for `a` occurrences in `c`
/// focus words and `b` in `d` reference words.
pub fn scores(a: usize, b: usize, c: usize, d: usize) -> (f64, f64, f64) {
    let (a, b, c, d) = (a as f64, b as f64, c as f64, d as f64);
    if c == 0.0 || d == 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let e1 = c * (a + b) / (c + d);
    let e2 = d * (a + b) / (c + d);
    let term = |o: f64, e: f64| if o > 0.0 && e > 0.0 { o * (o / e).ln() } else { 0.0 };
    let ll = 2.0 * (term(a, e1) + term(b, e2));
    let (fa, fb) = (a.max(0.5) / c, b.max(0.5) / d);
    let lr = (fa / fb).log2();
    let pd = (fa - fb) * 100.0 / fb;
    (if a / c < b / d { -ll } else { ll }, lr, pd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_match_worked_values() {
        // 20 of 1000 words in the text, 10 of 10,000 in the rest.
        let (ll, lr, pd) = scores(20, 10, 1000, 10_000);
        let e1 = 1000.0 * 30.0 / 11_000.0;
        let e2 = 10_000.0 * 30.0 / 11_000.0;
        let hand = 2.0 * (20.0 * (20.0f64 / e1).ln() + 10.0 * (10.0f64 / e2).ln());
        assert!((ll - hand).abs() < 1e-9, "{ll} {hand}");
        assert!((lr - (0.02f64 / 0.001).log2()).abs() < 1e-9);
        assert!((pd - 1900.0).abs() < 1e-9);
        assert!(scores(1, 100, 1000, 1000).0 < 0.0, "used less: negative");
        assert!(scores(3, 0, 100, 100).1.is_finite(), "a zero is taken as 0.5");
    }

    #[test]
    fn compares_a_text_with_the_rest() {
        let dir = std::env::temp_dir().join(format!("wugs-kw-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        let sample = crate::session::list_texts(&mut session).unwrap()[0].id.clone();
        let fox = crate::session::create_text(&mut session, "Fox").unwrap();
        for (i, line) in ["the fox drank", "the fox ran", "Fox slept"].iter().enumerate() {
            crate::session::add_segment(&mut session, &fox, None, Some(i as f64 * 1000.0), Some(i as f64 * 1000.0 + 900.0), line).unwrap();
        }
        let k = keywords(&mut session, Kind::Word, &fox, 1).unwrap();
        assert_eq!(k.focus_tokens, 8);
        assert!(k.reference_tokens > 0);
        assert_eq!(k.rows[0].item.to_lowercase(), "fox", "most key first");
        assert_eq!((k.rows[0].focus, k.rows[0].reference), (3, 0), "case folded");
        assert!(k.rows.last().unwrap().log_likelihood < 0.0, "the sample's words are negative keywords of «Fox»");
        assert!(keywords(&mut session, Kind::Word, &fox, 2).unwrap().rows.iter().all(|r| r.focus >= 2 || r.focus == 0));
        assert!(keywords(&mut session, Kind::Gloss, &sample, 1).is_err());
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
