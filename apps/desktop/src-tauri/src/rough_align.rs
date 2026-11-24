//! Time a typed transcription roughly against its recording: the sentences
//! of a track that has no times yet, in order, laid over the stretches of
//! speech the `segmentation` capability finds, so each sentence takes the
//! speech its length calls for. A first pass to check by ear and correct
//! on the Timeline, never a forced alignment.
//!
//! The fit is a dynamic programme like Gale and Church's (the `align`
//! module): a sentence takes one stretch, or two to four in a row, or two
//! or three sentences share one stretch (cut by their lengths), and a
//! stretch may go unused (a cough, a question from the room). Each choice
//! costs the squared log ratio of the speech it gets to the speech its
//! characters predict, plus a small cost for each join or share.

use serde::Serialize;

use crate::session::{self, Session, SegmentStateView};

/// At most this many sentences or stretches are fitted at once.
pub const MAX_SENTENCES: usize = 3000;
pub const MAX_SPANS: usize = 4000;
/// The most stretches one sentence takes, and sentences one stretch holds.
const MAX_JOIN: usize = 4;
const MAX_SHARE: usize = 3;
const JOIN_COST: f64 = 0.35;
const SHARE_COST: f64 = 0.6;
/// Leaving a stretch out costs a little, and more the longer it is: a
/// cough is cheap to drop, a sentence's worth of speech is not.
const SKIP_BASE: f64 = 0.1;
const SKIP_PER_AVERAGE: f64 = 2.0;

/// How one choice fits: squared log ratio of the speech given to the speech expected.
fn misfit(given: f64, expected: f64) -> f64 {
    let r = (given.max(1.0) / expected.max(1.0)).ln();
    r * r
}

/// How a cell was reached, one byte a cell (the table can hold millions).
#[derive(Clone, Copy, PartialEq)]
enum Step {
    None,
    /// One sentence over this many stretches.
    Join(u8),
    /// This many sentences in one stretch.
    Share(u8),
    /// A stretch left out.
    Skip,
}

/// Each sentence's span, given its weight (its letters) and the stretches
/// of speech (ms, in time order). Every sentence gets a span when there
/// are sentences and stretches; spans run forward and never overlap.
pub fn fit(weights: &[usize], spans: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let (n, m) = (weights.len(), spans.len());
    if n == 0 || m == 0 {
        return Vec::new();
    }
    let w: Vec<f64> = weights.iter().map(|&x| x.max(1) as f64).collect();
    let speech: f64 = spans.iter().map(|(s, e)| e - s).sum();
    let per = speech / w.iter().sum::<f64>();
    let mut wsum = vec![0.0; n + 1];
    for i in 0..n {
        wsum[i + 1] = wsum[i] + w[i];
    }
    let mut dsum = vec![0.0; m + 1];
    for j in 0..m {
        dsum[j + 1] = dsum[j] + (spans[j].1 - spans[j].0);
    }
    let avg = speech / m as f64;
    let cols = m + 1;
    let mut cost = vec![f32::INFINITY; (n + 1) * cols];
    let mut back = vec![Step::None; (n + 1) * cols];
    cost[0] = 0.0;
    for i in 0..=n {
        for j in 0..=m {
            let here = cost[i * cols + j];
            if !here.is_finite() {
                continue;
            }
            let mut relax = |ni: usize, nj: usize, c: f64, step: Step| {
                let k = ni * cols + nj;
                let total = here + c as f32;
                if total < cost[k] {
                    cost[k] = total;
                    back[k] = step;
                }
            };
            if j < m {
                relax(i, j + 1, SKIP_BASE + SKIP_PER_AVERAGE * (spans[j].1 - spans[j].0) / avg, Step::Skip);
            }
            if i < n {
                for k in 1..=MAX_JOIN.min(m - j) {
                    let given = dsum[j + k] - dsum[j];
                    relax(i + 1, j + k, misfit(given, w[i] * per) + JOIN_COST * (k - 1) as f64, Step::Join(k as u8));
                }
                if j < m {
                    for k in 2..=MAX_SHARE.min(n - i) {
                        let expected = (wsum[i + k] - wsum[i]) * per;
                        relax(i + k, j + 1, misfit(dsum[j + 1] - dsum[j], expected) + SHARE_COST * (k - 1) as f64, Step::Share(k as u8));
                    }
                }
            }
        }
    }
    let mut out = vec![(0.0, 0.0); n];
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        match back[i * cols + j] {
            Step::None => unreachable!("every cell on the path was reached"),
            Step::Skip => j -= 1,
            Step::Join(k) => {
                let k = k as usize;
                out[i - 1] = (spans[j - k].0, spans[j - 1].1);
                i -= 1;
                j -= k;
            }
            Step::Share(k) => {
                let k = k as usize;
                let (s, e) = spans[j - 1];
                let total = wsum[i] - wsum[i - k];
                let mut at = s;
                for q in 0..k {
                    let idx = i - k + q;
                    let end = if q + 1 == k { e } else { (at + (e - s) * w[idx] / total).round() };
                    out[idx] = (at, end);
                    at = end;
                }
                i -= k;
                j -= 1;
            }
        }
    }
    out
}

/// The sentences a rough alignment would time: every live segment of the
/// track with text, in order, when none of the track's segments has times.
#[derive(Debug)]
pub struct Sentences {
    pub tier: String,
    pub tier_name: String,
    pub rows: Vec<(String, String)>,
}

pub fn sentences(session: &mut Session, text: &str, tier: Option<&str>) -> Result<Sentences, String> {
    let p = session::project(session)?;
    let tier = match tier {
        Some(t) => t.to_string(),
        None => session::utterance_tier(p)?,
    };
    let tier_name = p.proj.field_str(&tier, "name")?.unwrap_or_default();
    let mut rows = Vec::new();
    for id in p.proj.ordered_children("text", text, "segment")? {
        if p.proj.is_tombstoned(&id)? || p.proj.field_str(&id, "tier")?.as_deref() != Some(tier.as_str()) {
            continue;
        }
        if p.proj.field_num(&id, "start")?.is_some() || p.proj.field_num(&id, "end")?.is_some() {
            return Err(format!("«{tier_name}» already has timed segments. Rough timing is for a transcription typed before it was timed; time the rest by hand."));
        }
        let content = p.proj.field_str(&id, "content")?.unwrap_or_default();
        if !content.trim().is_empty() {
            rows.push((id, content));
        }
    }
    if rows.is_empty() {
        return Err(format!("«{tier_name}» has no sentences without times to place."));
    }
    if rows.len() > MAX_SENTENCES {
        return Err(format!("«{tier_name}» has {} sentences; rough timing takes at most {MAX_SENTENCES} at once.", rows.len()));
    }
    Ok(Sentences { tier, tier_name, rows })
}

/// A sentence's weight: its letters and digits (spaces and punctuation take no time).
pub fn weight(content: &str) -> usize {
    content.chars().filter(|c| c.is_alphanumeric()).count()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timed {
    pub tier: String,
    pub tier_name: String,
    pub sentences: usize,
    pub spans: usize,
    /// The segments as written, for the undo's check.
    pub states: Vec<SegmentStateView>,
}

/// Write the fitted spans in one resolving txn, after checking again that
/// the track has no times (the spans were found off the session thread).
pub fn write(session: &mut Session, text: &str, tier: &str, spans: &[(f64, f64)]) -> Result<Timed, String> {
    if spans.is_empty() {
        return Err("No speech was found in this recording.".into());
    }
    if spans.len() > MAX_SPANS {
        return Err(format!("The recording has {} stretches of speech; rough timing takes at most {MAX_SPANS}. Try a shorter recording.", spans.len()));
    }
    let found = sentences(session, text, Some(tier))?;
    let weights: Vec<usize> = found.rows.iter().map(|(_, c)| weight(c)).collect();
    let times = fit(&weights, spans);
    let rows: Vec<(String, Option<(f64, f64)>)> = found.rows.iter().zip(&times).map(|((id, _), &t)| (id.clone(), Some(t))).collect();
    let p = session::project(session)?;
    p.time_segments(text, &rows)?;
    let ids: Vec<String> = rows.into_iter().map(|(id, _)| id).collect();
    Ok(Timed { tier: found.tier, tier_name: found.tier_name, sentences: ids.len(), spans: spans.len(), states: session::states(p, &ids)? })
}

/// The undo: take the times away again, only while every segment still has
/// the times it was given (else nothing is written).
pub fn untime(session: &mut Session, text: &str, expect: &[session::SegmentCheckView]) -> Result<(), String> {
    let p = session::project(session)?;
    let checks: Vec<wugs_core::SegmentCheck> = expect.iter().map(|c| wugs_core::SegmentCheck { id: c.id.clone(), start: c.start_ms, end: c.end_ms, ..Default::default() }).collect();
    p.require_unchanged(&checks)?;
    let rows: Vec<(String, Option<(f64, f64)>)> = expect.iter().map(|c| (c.id.clone(), None)).collect();
    p.time_segments(text, &rows).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_sentence_a_stretch_when_they_match() {
        let spans = [(100.0, 1100.0), (1500.0, 2000.0), (2600.0, 4600.0)];
        assert_eq!(fit(&[20, 10, 40], &spans), spans.to_vec());
    }

    #[test]
    fn a_long_sentence_takes_the_stretches_its_pause_split() {
        // The second sentence was spoken with a pause in it.
        let spans = [(0.0, 1000.0), (1400.0, 2400.0), (2600.0, 3600.0), (4000.0, 5000.0)];
        assert_eq!(fit(&[10, 20, 10], &spans), vec![(0.0, 1000.0), (1400.0, 3600.0), (4000.0, 5000.0)]);
    }

    #[test]
    fn two_short_sentences_share_a_stretch_said_without_a_pause() {
        let spans = [(0.0, 2000.0), (2500.0, 4500.0)];
        let t = fit(&[10, 10, 20], &spans);
        assert_eq!(t, vec![(0.0, 1000.0), (1000.0, 2000.0), (2500.0, 4500.0)]);
    }

    #[test]
    fn a_cough_is_left_out() {
        let spans = [(0.0, 2000.0), (2300.0, 2400.0), (3000.0, 5000.0)];
        assert_eq!(fit(&[20, 20], &spans), vec![(0.0, 2000.0), (3000.0, 5000.0)]);
    }

    #[test]
    fn every_sentence_gets_a_forward_span_in_order() {
        let spans: Vec<(f64, f64)> = (0..37).map(|i| (i as f64 * 1000.0, i as f64 * 1000.0 + 600.0 + (i % 5) as f64 * 70.0)).collect();
        let weights: Vec<usize> = (0..50).map(|i| 5 + (i * 7) % 23).collect();
        let t = fit(&weights, &spans);
        assert_eq!(t.len(), 50);
        for w in t.windows(2) {
            assert!(w[0].0 < w[0].1 && w[0].1 <= w[1].0, "{w:?}");
        }
        assert!(fit(&[], &spans).is_empty() && fit(&[3], &[]).is_empty());
    }

    #[test]
    fn the_largest_fit_is_quick() {
        let spans: Vec<(f64, f64)> = (0..MAX_SPANS).map(|i| (i as f64 * 1000.0, i as f64 * 1000.0 + 700.0)).collect();
        let weights: Vec<usize> = (0..MAX_SENTENCES).map(|i| 10 + i % 17).collect();
        let at = std::time::Instant::now();
        assert_eq!(fit(&weights, &spans).len(), MAX_SENTENCES);
        assert!(at.elapsed().as_secs() < 30, "{:?}", at.elapsed());
    }

    #[test]
    fn a_typed_text_is_timed_and_untimed() {
        let dir = std::env::temp_dir().join(format!("wugs-rough-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let p = session::project(&mut s).unwrap();
        let tier = session::utterance_tier(p).unwrap();
        let (text, _) = p.import_written_text("Story", &tier, &["Mtu mmoja alikwenda sokoni.", "Alinunua ndizi.", "Kisha akarudi nyumbani."]).unwrap();
        let r = write(&mut s, &text, &tier, &[(500.0, 2500.0), (3000.0, 4100.0), (4600.0, 6200.0)]).unwrap();
        assert_eq!((r.sentences, r.spans), (3, 3));
        assert_eq!((r.states[1].start_ms, r.states[1].end_ms), (Some(3000.0), Some(4100.0)));
        assert!(sentences(&mut s, &text, Some(&tier)).unwrap_err().contains("already has timed"), "a second run is refused");
        let checks: Vec<session::SegmentCheckView> = r.states.iter().map(|st| session::SegmentCheckView { id: st.id.clone(), start_ms: st.start_ms, end_ms: st.end_ms, ..Default::default() }).collect();
        untime(&mut s, &text, &checks).unwrap();
        assert_eq!(sentences(&mut s, &text, Some(&tier)).unwrap().rows.len(), 3, "untimed again");
        std::fs::remove_dir_all(&dir).ok();
    }
}
