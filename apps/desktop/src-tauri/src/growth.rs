//! Vocabulary growth: how many different words (types) have been met after
//! each running word (token), reading the texts in the order they were made.
//! A curve still climbing steeply means each new recording still brings new
//! words; a flattening one means the vocabulary of this kind of text is
//! mostly in. Words are counted as the Frequencies panel counts them (NFC,
//! case folded, diacritics kept), on transcription tracks.

use std::collections::HashMap;

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{project, Session};

/// At most this many points go to the chart.
pub const MAX_POINTS: usize = 400;
/// Marks kept while reading; past this, every other one is dropped and the
/// step between marks doubles, so memory stays bounded on any corpus.
const MAX_MARKS: usize = 8192;

#[derive(Serialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GrowthPoint {
    pub tokens: usize,
    pub types: usize,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextStart {
    pub title: String,
    /// Running words before the text's first word.
    pub at: usize,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Growth {
    /// From (0, 0) to (tokens, types), at most `MAX_POINTS` + 1.
    pub points: Vec<GrowthPoint>,
    pub tokens: usize,
    pub types: usize,
    /// Each text with words, where it starts.
    pub texts: Vec<TextStart>,
    /// Word types met once and twice: Good-Turing's chance that the next word
    /// is new is `once / tokens`, and Chao1's estimate of the vocabulary uses both.
    pub once: usize,
    pub twice: usize,
}

pub fn vocabulary_growth(session: &mut Session) -> Result<Growth, String> {
    let p = project(session)?;
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut tokens = 0usize;
    let mut marks: Vec<GrowthPoint> = vec![GrowthPoint { tokens: 0, types: 0 }];
    let mut texts: Vec<TextStart> = Vec::new();
    let mut last_text = String::new();
    let mut step = 1usize;
    each_segment(p, Read::Words, &Scope::all(), |seg| {
        for w in &seg.words {
            let key = wugs_model::fold_key(&p.proj.nfc(w));
            if key.is_empty() {
                continue;
            }
            if seg.text != last_text {
                last_text = seg.text.clone();
                texts.push(TextStart { title: seg.title.clone(), at: tokens });
            }
            tokens += 1;
            *seen.entry(key).or_insert(0) += 1;
            if tokens % step == 0 {
                marks.push(GrowthPoint { tokens, types: seen.len() });
                if marks.len() > MAX_MARKS {
                    marks = std::mem::take(&mut marks).into_iter().step_by(2).collect();
                    step *= 2;
                }
            }
        }
        Ok(())
    })?;
    if marks.last().map(|m| m.tokens) != Some(tokens) {
        marks.push(GrowthPoint { tokens, types: seen.len() });
    }
    let once = seen.values().filter(|&&n| n == 1).count();
    let twice = seen.values().filter(|&&n| n == 2).count();
    Ok(Growth { points: thin(&marks, MAX_POINTS), tokens, types: seen.len(), texts, once, twice })
}

/// At most `max` + 1 points, evenly spread by tokens, always the first and last.
fn thin(marks: &[GrowthPoint], max: usize) -> Vec<GrowthPoint> {
    if marks.len() <= max + 1 {
        return marks.to_vec();
    }
    let last = marks.len() - 1;
    let mut out: Vec<GrowthPoint> = (0..=max).map(|i| marks[i * last / max]).collect();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinning_keeps_both_ends() {
        let marks: Vec<_> = (0..1000).map(|i| GrowthPoint { tokens: i * 16, types: i }).collect();
        let t = thin(&marks, 10);
        assert_eq!(t.len(), 11);
        assert_eq!(t[0], marks[0]);
        assert_eq!(t[10], marks[999]);
        assert_eq!(thin(&marks[..5], 10).len(), 5);
    }

    #[test]
    fn types_grow_with_the_texts() {
        let dir = std::env::temp_dir().join(format!("wugs-growth-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let p = project(&mut s).unwrap();
        let tier = crate::session::utterance_tier(p).unwrap();
        let a = p.create_text("One").unwrap();
        p.add_segment(&a, &tier, 0, None, None, "ka ka Mi").unwrap();
        let b = p.create_text("Two").unwrap();
        p.add_segment(&b, &tier, 0, None, None, "mi to").unwrap();
        let g = vocabulary_growth(&mut s).unwrap();
        assert_eq!((g.tokens, g.types), (5, 3));
        // ka and mi (Mi folded) twice, to once.
        assert_eq!((g.once, g.twice), (1, 2));
        assert_eq!(g.points.first(), Some(&GrowthPoint { tokens: 0, types: 0 }));
        let types: Vec<_> = g.points.iter().map(|p| p.types).collect();
        assert_eq!(types, [0, 1, 1, 2, 2, 3]);
        assert_eq!(g.texts, vec![TextStart { title: "One".into(), at: 0 }, TextStart { title: "Two".into(), at: 3 }]);
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
