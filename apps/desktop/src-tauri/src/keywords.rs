//! Keywords (Scrolls, SCROLLS.md §5 step 6, started early): which words one
//! text uses more, or less, than the rest of the project: a focus corpus
//! against a reference corpus, as AntConc's Keyword List and Sketch
//! Engine's keywords set it up. The focus is one text; the reference is every other
//! text in the project (a reference corpus from a dataset package comes with
//! the index, row 37).
//!
//! Words are counted under the Concordance's keys (`fold_key`), each row
//! showing the spelling met most often. For each word met in either corpus:
//! - **Log-likelihood**, the 4-term form over the whole 2×2 table (AntConc's
//!   default; Rayson's online calculator gives the 2-term form, a little
//!   lower), signed: positive when the text uses the word more than the
//!   reference, negative when less. Marked significant at p < 0.05 after a
//!   Bonferroni correction for the number of words tested, AntConc's default
//!   (`critical`).
//! - **Log ratio** (Hardie 2014): log2 of the ratio of relative frequencies,
//!   an effect size (1 = twice as often). A zero count is taken as 0.5 so
//!   words missing from one side get a finite value.
//! - **%DIFF** (Gabrielatos and Marchi 2012): the difference of relative
//!   frequencies as a percentage of the reference's, with the same 0.5.
//! - **Simple maths** (Kilgarriff 2009), Sketch Engine's keyword score:
//!   (per million here + 1) / (per million in the rest + 1).
//!
//! The minimum count applies to the side a word leans to: a word used more
//! here needs that many uses here, one used less needs that many in the rest.

use std::collections::HashMap;

use serde::Serialize;

use crate::concordance::{Read, each_segment, Kind};
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
    pub simple_maths: f64,
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
    /// Rows past the limit left out (the weakest of each side).
    pub omitted: usize,
    /// The log-likelihood a row needs for p < 0.05 after Bonferroni over `types`.
    pub critical: f64,
}

pub const LIMIT: usize = 2000;

#[derive(Default)]
struct Tally {
    focus: usize,
    reference: usize,
    spellings: Vec<(String, usize)>,
}

pub fn keywords(session: &mut Session, kind: Kind, text: &str, min_count: usize) -> Result<Keywords, String> {
    if !matches!(kind, Kind::Word | Kind::Translation) {
        return Err("keywords are counted between words or translation words".into());
    }
    let p = project(session)?;
    let mut tallies: HashMap<String, Tally> = HashMap::new();
    let (mut c, mut d) = (0usize, 0usize);
    each_segment(p, Read::of(kind), None, |seg| {
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
    let min = min_count.max(1);
    let (mut more, mut less): (Vec<KeywordRow>, Vec<KeywordRow>) = (Vec::new(), Vec::new());
    for (key, t) in tallies {
        let leans_here = (t.focus as f64) * (d as f64) >= (t.reference as f64) * (c as f64);
        if (leans_here && t.focus < min) || (!leans_here && t.reference < min) {
            continue;
        }
        let (ll, lr, pd) = scores(t.focus, t.reference, c, d);
        let sm = (t.focus as f64 * 1e6 / c.max(1) as f64 + 1.0) / (t.reference as f64 * 1e6 / d.max(1) as f64 + 1.0);
        let item = t.spellings.iter().fold(None::<&(String, usize)>, |best, s| if best.is_none_or(|b| s.1 > b.1) { Some(s) } else { best }).map(|s| s.0.clone()).unwrap_or(key);
        let row = KeywordRow { item, focus: t.focus, reference: t.reference, log_likelihood: ll, log_ratio: lr, percent_diff: pd, simple_maths: sm };
        if leans_here {
            more.push(row)
        } else {
            less.push(row)
        }
    }
    more.sort_by(|a, b| b.log_likelihood.total_cmp(&a.log_likelihood).then(a.item.cmp(&b.item)));
    less.sort_by(|a, b| a.log_likelihood.total_cmp(&b.log_likelihood).then(a.item.cmp(&b.item)));
    // Each side keeps its strongest; one side's unused share goes to the other.
    let keep_more = more.len().min((LIMIT / 2).max(LIMIT.saturating_sub(less.len())));
    let keep_less = less.len().min(LIMIT - keep_more);
    let omitted = more.len() - keep_more + less.len() - keep_less;
    more.truncate(keep_more);
    less.truncate(keep_less);
    less.reverse();
    let mut rows = more;
    rows.extend(less);
    let critical = chi2_critical(0.05 / types.max(1) as f64);
    Ok(Keywords { rows, focus_tokens: c, reference_tokens: d, types, omitted, critical })
}

/// Signed log-likelihood, log ratio and %DIFF for `a` occurrences in `c`
/// focus words and `b` in `d` reference words.
pub fn scores(a: usize, b: usize, c: usize, d: usize) -> (f64, f64, f64) {
    let (a, b, c, d) = (a as f64, b as f64, c as f64, d as f64);
    if c == 0.0 || d == 0.0 {
        return (0.0, 0.0, 0.0);
    }
    // The whole 2×2 table: the word and every other word, here and in the rest.
    let n = c + d;
    let (k, other) = (a + b, n - a - b);
    let term = |o: f64, e: f64| if o > 0.0 && e > 0.0 { o * (o / e).ln() } else { 0.0 };
    let ll = 2.0 * (term(a, c * k / n) + term(b, d * k / n) + term(c - a, c * other / n) + term(d - b, d * other / n));
    let (fa, fb) = (a.max(0.5) / c, b.max(0.5) / d);
    let lr = (fa / fb).log2();
    let pd = (fa - fb) * 100.0 / fb;
    (if a / c < b / d { -ll } else { ll }, lr, pd)
}

/// The chi-square (1 degree of freedom) value a statistic must reach for
/// significance level `alpha`: the square of the normal quantile at 1 − α/2.
pub fn chi2_critical(alpha: f64) -> f64 {
    let z = normal_quantile(1.0 - alpha / 2.0);
    z * z
}

/// The standard normal quantile (Acklam's rational approximation, relative
/// error under 1.2e-9).
fn normal_quantile(p: f64) -> f64 {
    const A: [f64; 6] = [-3.969683028665376e1, 2.209460984245205e2, -2.759285104469687e2, 1.383577518672690e2, -3.066479806614716e1, 2.506628277459239];
    const B: [f64; 5] = [-5.447609879822406e1, 1.615858368580409e2, -1.556989798598866e2, 6.680131188771972e1, -1.328068155288572e1];
    const C: [f64; 6] = [-7.784894002430293e-3, -3.223964580411365e-1, -2.400758277161838, -2.549732539343734, 4.374664141464968, 2.938163982698783];
    const D: [f64; 4] = [7.784695709041462e-3, 3.224671290700398e-1, 2.445134137142996, 3.754408661907416];
    let p = p.clamp(1e-300, 1.0 - 1e-16);
    let tail = |q: f64| (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5]) / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0);
    if p < 0.02425 {
        tail((-2.0 * p.ln()).sqrt())
    } else if p > 1.0 - 0.02425 {
        -tail((-2.0 * (1.0 - p).ln()).sqrt())
    } else {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_match_worked_values() {
        // 20 of 1000 words in the text, 10 of 10,000 in the rest.
        let (ll, lr, pd) = scores(20, 10, 1000, 10_000);
        let (n, k) = (11_000.0f64, 30.0f64);
        let t = |o: f64, e: f64| o * (o / e).ln();
        let hand = 2.0 * (t(20.0, 1000.0 * k / n) + t(10.0, 10_000.0 * k / n) + t(980.0, 1000.0 * (n - k) / n) + t(9990.0, 10_000.0 * (n - k) / n));
        assert!((ll - hand).abs() < 1e-9, "{ll} {hand}");
        assert!((lr - (0.02f64 / 0.001).log2()).abs() < 1e-9);
        assert!((pd - 1900.0).abs() < 1e-9);
        assert!(scores(1, 100, 1000, 1000).0 < 0.0, "used less: negative");
        assert!(scores(3, 0, 100, 100).1.is_finite(), "a zero is taken as 0.5");
        // Bonferroni critical values (chi-square, 1 df).
        for (types, crit) in [(1usize, 3.8415), (100, 12.116), (1000, 16.448), (20_000, 22.1665)] {
            assert!((chi2_critical(0.05 / types as f64) - crit).abs() < 2e-3, "{types}");
        }
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
        // The minimum applies to the side a word leans to: a word used less
        // here survives on its count in the rest.
        let two = keywords(&mut session, Kind::Word, &fox, 2).unwrap();
        assert!(two.rows.iter().all(|r| if r.log_likelihood >= 0.0 { r.focus >= 2 } else { r.reference >= 2 }));
        assert!(two.critical > 3.84);
        assert!(keywords(&mut session, Kind::Gloss, &sample, 1).is_err());
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
