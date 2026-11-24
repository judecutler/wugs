//! Segment timing checks (the Checks panel's Segment timing): segments on
//! one transcription track that overlap, and ones so short they are likely
//! a stray click or so long that ELAN, Praat and speech tools handle them
//! badly (over 30 s). Read through the corpus tools' one reader; untimed
//! segments are not checked.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{project, Session};

/// Shorter than this is listed as short.
pub const SHORT_MS: f64 = 150.0;
/// Longer than this is listed as long (what speech tools take at most).
pub const LONG_MS: f64 = 30_000.0;
/// At most this many are listed; `counts` has every one.
const MAX_LISTED: usize = 500;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimingIssue {
    /// `overlap`, `short` or `long`.
    pub kind: &'static str,
    pub text_id: String,
    pub text_title: String,
    pub tier_id: String,
    pub segment_id: String,
    pub sentence: String,
    pub start_ms: f64,
    pub end_ms: f64,
    /// For an overlap: the earlier segment it runs into, and by how much.
    pub other: Option<String>,
    pub overlap_ms: Option<f64>,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Timing {
    /// Timed segments checked.
    pub timed: usize,
    pub overlaps: usize,
    pub short: usize,
    pub long: usize,
    pub issues: Vec<TimingIssue>,
}

pub fn timing(session: &mut Session) -> Result<Timing, String> {
    let p = project(session)?;
    let mut out = Timing::default();
    // Each (text, tier)'s timed segments, gathered first, then checked in time order.
    let mut tracks: BTreeMap<(String, String), Vec<(f64, f64, String, String, String)>> = BTreeMap::new();
    each_segment(p, Read::Words, &Scope::all(), |seg| {
        let (Some(start), Some(end)) = (p.proj.field_num(&seg.id, "start")?, p.proj.field_num(&seg.id, "end")?) else { return Ok(()) };
        tracks.entry((seg.text.clone(), seg.tier.clone())).or_default().push((start, end, seg.id.clone(), seg.title.clone(), seg.words.join(" ")));
        Ok(())
    })?;
    let mut issues = Vec::new();
    for ((text, tier), mut segs) in tracks {
        segs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut reach: Option<(f64, String)> = None;
        for (start, end, id, title, sentence) in segs {
            out.timed += 1;
            let issue = |kind, other: Option<String>, overlap_ms| TimingIssue { kind, text_id: text.clone(), text_title: title.clone(), tier_id: tier.clone(), segment_id: id.clone(), sentence: sentence.clone(), start_ms: start, end_ms: end, other, overlap_ms };
            if let Some((until, other)) = &reach {
                if start < *until {
                    out.overlaps += 1;
                    issues.push(issue("overlap", Some(other.clone()), Some(until.min(end) - start)));
                }
            }
            if end - start < SHORT_MS {
                out.short += 1;
                issues.push(issue("short", None, None));
            } else if end - start > LONG_MS {
                out.long += 1;
                issues.push(issue("long", None, None));
            }
            if reach.as_ref().is_none_or(|(until, _)| end > *until) {
                reach = Some((end, id.clone()));
            }
        }
    }
    out.issues = issues.into_iter().take(MAX_LISTED).collect();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlaps_short_and_long_segments_are_listed() {
        let dir = std::env::temp_dir().join(format!("wugs-timing-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let text = crate::session::create_text(&mut s, "Timing").unwrap();
        for (a, b, words) in [(0.0, 1000.0, "one"), (800.0, 2000.0, "two"), (2500.0, 2550.0, "click"), (3000.0, 40_000.0, "a long story"), (41_000.0, 42_000.0, "fine")] {
            crate::session::add_segment(&mut s, &text, None, Some(a), Some(b), words).unwrap();
        }
        crate::session::add_segment(&mut s, &text, None, None, None, "untimed").unwrap();
        let t = timing(&mut s).unwrap();
        assert_eq!((t.timed, t.overlaps, t.short, t.long), (5, 1, 1, 1));
        let o = t.issues.iter().find(|x| x.kind == "overlap").unwrap();
        assert_eq!((o.sentence.as_str(), o.overlap_ms), ("two", Some(200.0)));
        assert!(t.issues.iter().any(|x| x.kind == "short" && x.sentence == "click"));
        assert!(t.issues.iter().any(|x| x.kind == "long" && x.sentence == "a long story"));
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
