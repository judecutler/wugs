// Where in the corpus an item occurs (roadmap 41, AntConc's Plot): for each
// text, its running words and the word positions of every match, and how
// evenly the matches are spread over the texts as Gries's DP (deviation of
// proportions, Gries 2008; normalised as Lijffijt & Gries 2012). Matches are
// met by the Concordance's own rule (`concordance::scan`), so the hits here
// are the Concordance's lines.
use serde::Serialize;

use crate::concordance::{scan, Kind, Match, Scope};
use crate::session::{project, Session};

/// At most this many positions are sent for one text; the count is still whole.
pub const POSITIONS: usize = 5000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPlot {
    pub text_id: String,
    pub title: String,
    /// Running words of the text the item could be met in.
    pub words: usize,
    pub hits: usize,
    /// Word offsets of the matches within the text, in order (at most `POSITIONS`).
    pub positions: Vec<usize>,
    /// The text's details (genre, speaker…), for the plot's "by detail" view.
    pub meta: Vec<(String, String)>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dispersion {
    pub texts: Vec<TextPlot>,
    pub words: usize,
    pub hits: usize,
    /// Gries's DP: 0 when the matches follow the texts' sizes exactly, near 1
    /// when they sit in one small text. None with no matches or one text.
    pub dp: Option<f64>,
    /// DP divided by its largest possible value for these text sizes.
    pub dp_norm: Option<f64>,
}

/// DP and its normalised form over parts of `sizes` words holding `hits` matches.
pub fn dp(sizes: &[usize], hits: &[usize]) -> (Option<f64>, Option<f64>) {
    let total_words: usize = sizes.iter().sum();
    let total_hits: usize = hits.iter().sum();
    let parts = sizes.iter().filter(|s| **s > 0).count();
    if total_hits == 0 || total_words == 0 || parts < 2 {
        return (None, None);
    }
    let mut d = 0.0;
    let mut smallest = 1.0f64;
    for (s, h) in sizes.iter().zip(hits) {
        if *s == 0 {
            continue;
        }
        let expected = *s as f64 / total_words as f64;
        smallest = smallest.min(expected);
        d += (*h as f64 / total_hits as f64 - expected).abs();
    }
    let d = d / 2.0;
    let most = 1.0 - smallest;
    (Some(d), if most > 0.0 { Some(d / most) } else { None })
}

pub fn dispersion(session: &mut Session, query: &str, kind: Kind, how: Match, scope: &Scope) -> Result<Dispersion, String> {
    let p = project(session)?;
    let mut texts: Vec<TextPlot> = Vec::new();
    scan(p, query, kind, how, scope, |seg, found| {
        if texts.last().is_none_or(|t| t.text_id != seg.text) {
            texts.push(TextPlot { text_id: seg.text.clone(), title: seg.title.clone(), words: 0, hits: 0, positions: Vec::new(), meta: p.text_metadata(&seg.text)? });
        }
        let t = texts.last_mut().expect("pushed above");
        for f in &found {
            t.hits += 1;
            if t.positions.len() < POSITIONS {
                t.positions.push(t.words + f.at);
            }
        }
        t.words += seg.words.len();
        Ok(())
    })?;
    let sizes: Vec<usize> = texts.iter().map(|t| t.words).collect();
    let counts: Vec<usize> = texts.iter().map(|t| t.hits).collect();
    let (dp, dp_norm) = dp(&sizes, &counts);
    Ok(Dispersion { words: sizes.iter().sum(), hits: counts.iter().sum(), dp, dp_norm, texts })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dp_follows_gries() {
        // Gries (2008)'s worked example: five parts of 9, 10, 10, 10, 11 words, hits 1, 2, 3, 4, 5.
        let (d, n) = dp(&[9, 10, 10, 10, 11], &[1, 2, 3, 4, 5]);
        let d = d.unwrap();
        // Expected shares .18 .2 .2 .2 .22; observed 1/15 … 5/15.
        let want = 0.5 * ((1.0 / 15.0 - 0.18f64).abs() + (2.0 / 15.0 - 0.2f64).abs() + (3.0 / 15.0 - 0.2f64).abs() + (4.0 / 15.0 - 0.2f64).abs() + (5.0 / 15.0 - 0.22f64).abs());
        assert!((d - want).abs() < 1e-9, "{d} against {want}");
        assert!((n.unwrap() - want / (1.0 - 0.18)).abs() < 1e-9);
        // Proportional to size: no deviation. All in one part: the most there can be.
        assert_eq!(dp(&[10, 30], &[1, 3]).0, Some(0.0));
        let (d, n) = dp(&[10, 30], &[4, 0]);
        assert!((d.unwrap() - 0.75).abs() < 1e-9);
        assert!((n.unwrap() - 1.0).abs() < 1e-9);
        // Nothing to compare.
        assert_eq!(dp(&[10], &[3]), (None, None));
        assert_eq!(dp(&[10, 10], &[0, 0]), (None, None));
    }

    #[test]
    fn plots_the_concordance_lines_in_the_sample() {
        let dir = std::env::temp_dir().join(format!("wugs-dispersion-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        crate::sample::add_morphology(&mut session).unwrap();
        for (q, kind) in [("t*", Kind::Word), ("kani", Kind::Morph), ("t*", Kind::Translation)] {
            let lines = crate::concordance::concordance_in(&mut session, q, kind, Match::Glob, &Scope::all()).unwrap();
            let d = dispersion(&mut session, q, kind, Match::Glob, &Scope::all()).unwrap();
            assert_eq!(d.hits, lines.total, "{q}");
            assert_eq!(d.texts.iter().map(|t| t.positions.len()).sum::<usize>(), lines.total);
            for t in &d.texts {
                assert!(t.positions.iter().all(|x| *x < t.words), "{q}: a position past the text's words");
                assert!(t.positions.windows(2).all(|w| w[0] <= w[1]));
            }
        }
        let d = dispersion(&mut session, "tuː", Kind::Word, Match::Exact, &Scope::all()).unwrap();
        assert_eq!(d.hits, 1);
        // «wʌn tuː θɹiː»: the second word of the text.
        let t = d.texts.iter().find(|t| t.hits == 1).unwrap();
        assert_eq!(t.positions, vec![1]);
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
