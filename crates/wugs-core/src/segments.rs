//! Timeline cutting (roadmap 59): machine segmentation, split, merge and
//! restore. Each is one txn, so it lands whole or not at all, and each is
//! an ordinary write: an undo in the shell is the inverse written as a new
//! edit (a split is undone by a merge, a delete by a restore), never a
//! rewrite of the log.

use wugs_model::{pos_between, pos_seq, Value};

use crate::{CommitResult, Project};

/// The intent machine-proposed segments are written under, so the log says
/// which boundaries a person drew and which a segmenter proposed.
pub const MACHINE_SEGMENT_INTENT: &str = "machine-segment";

/// A segment's time span, when it has both edges.
fn span(p: &Project, seg: &str) -> Result<Option<(f64, f64)>, String> {
    Ok(match (p.proj.field_num(seg, "start")?, p.proj.field_num(seg, "end")?) {
        (Some(s), Some(e)) => Some((s, e)),
        _ => None,
    })
}

impl Project {
    /// Live timed segments of `text` on `tier`, as (id, start, end) in time
    /// order (ties by id, so every device sees one order).
    fn tier_spans(&self, text: &str, tier: &str) -> Result<Vec<(String, f64, f64)>, String> {
        let mut out = Vec::new();
        for id in self.proj.children_by_ref("text", text, "segment")? {
            if self.proj.field_str(&id, "tier")?.as_deref() != Some(tier) {
                continue;
            }
            if let Some((s, e)) = span(self, &id)? {
                out.push((id, s, e));
            }
        }
        out.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        Ok(out)
    }

    /// Write one segment per span on `tier`, in one txn under
    /// [`MACHINE_SEGMENT_INTENT`], skipping any span that overlaps a live
    /// segment already on the tier (a person's boundaries are never
    /// overridden) or an earlier span of this call. Positions run from
    /// `first_index` in time order. The segments are ordinary ones: editable,
    /// movable, deletable. Returns the new ids (empty when every span was
    /// skipped; nothing is written then).
    pub fn add_segments(&mut self, text: &str, tier: &str, first_index: usize, spans: &[(f64, f64)]) -> Result<(Vec<String>, CommitResult), String> {
        self.require(text, "text")?;
        self.require(tier, "tier")?;
        let mut taken: Vec<(f64, f64)> = self.tier_spans(text, tier)?.into_iter().map(|(_, s, e)| (s, e)).collect();
        let mut wanted: Vec<(f64, f64)> = spans.iter().copied().filter(|&(s, e)| s.is_finite() && e.is_finite() && s >= 0.0 && e > s).collect();
        wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut t = self.intent(MACHINE_SEGMENT_INTENT);
        let mut ids = Vec::new();
        for (s, e) in wanted {
            if taken.iter().any(|&(ts, te)| s < te && ts < e) {
                continue;
            }
            taken.push((s, e));
            let id = t.create(
                "segment",
                "seg",
                text,
                vec![
                    ("text", Value::str(text)),
                    ("tier", Value::str(tier)),
                    ("pos", Value::str(&pos_seq(first_index + ids.len()))),
                    ("content", Value::str("")),
                    ("start", Value::Num(s)),
                    ("end", Value::Num(e)),
                ],
            );
            ids.push(id);
        }
        let res = t.commit()?;
        Ok((ids, res))
    }

    /// Bring a deleted segment back (the undo of a delete), with the fields
    /// it had. One `restore` op under `restore-segment`.
    pub fn restore_segment(&mut self, text: &str, segment: &str) -> Result<CommitResult, String> {
        self.restore_segments(text, &[segment.to_string()])
    }

    /// Bring deleted segments back in one txn (the undo of an Auto-segment
    /// undo, or of a delete). Every one must be a deleted segment of `text`.
    pub fn restore_segments(&mut self, text: &str, segments: &[String]) -> Result<CommitResult, String> {
        self.require(text, "text")?;
        for seg in segments {
            if self.proj.entity_kind(seg)?.as_deref() != Some("segment") {
                return Err(format!("no such segment: {seg}"));
            }
            if self.proj.field_str(seg, "text")?.as_deref() != Some(text) {
                return Err(format!("segment {seg} does not belong to text {text}"));
            }
            if !self.proj.is_tombstoned(seg)? {
                return Err(format!("segment {seg} is not deleted"));
            }
        }
        let mut t = self.intent("restore-segment");
        for seg in segments {
            t.restore("segment", seg, text);
        }
        t.commit()
    }

    /// Delete several segments in one txn (the undo of an Auto-segment).
    pub fn delete_segments(&mut self, text: &str, segments: &[String]) -> Result<CommitResult, String> {
        for seg in segments {
            self.require_in_text(seg, "segment", text)?;
        }
        let mut t = self.intent("delete-segment");
        for seg in segments {
            t.delete("segment", seg, text);
        }
        t.commit()
    }

    /// Cut a segment in two at `at_ms`: its end becomes `at_ms` and a new
    /// segment on the same tier runs from `at_ms` to the old end, placed
    /// right after it in `pos` order. The transcription, its words and
    /// everything else stay on the first part; the new part starts empty,
    /// because no rule can tell which words were said after the cut. One
    /// resolving txn (a person's direct edit, like a retime). Returns the
    /// new segment's id.
    pub fn split_segment(&mut self, text: &str, segment: &str, at_ms: f64) -> Result<(String, CommitResult), String> {
        self.require_in_text(segment, "segment", text)?;
        let (start, end) = span(self, segment)?.ok_or_else(|| format!("segment {segment} has no times to split"))?;
        if !(at_ms.is_finite() && at_ms > start && at_ms < end) {
            return Err(format!("{at_ms} ms is not inside the segment ({start} to {end} ms)"));
        }
        let tier = self.proj.field_str(segment, "tier")?.unwrap_or_default();
        let pos = self.proj.field_str(segment, "pos")?.unwrap_or_default();
        let mut next: Option<String> = None;
        for sib in self.proj.children_by_ref("text", text, "segment")? {
            let p = self.proj.field_str(&sib, "pos")?.unwrap_or_default();
            if p > pos && next.as_ref().is_none_or(|n| p < *n) {
                next = Some(p);
            }
        }
        let new_pos = pos_between(&pos, next.as_deref());
        let mut t = self.intent("split-segment").resolving();
        t.set("segment", segment, text, "end", Value::Num(at_ms))?;
        let id = t.create(
            "segment",
            "seg",
            text,
            vec![
                ("text", Value::str(text)),
                ("tier", Value::str(&tier)),
                ("pos", Value::str(&new_pos)),
                ("content", Value::str("")),
                ("start", Value::Num(at_ms)),
                ("end", Value::Num(end)),
            ],
        );
        let res = t.commit()?;
        Ok((id, res))
    }

    /// Join `second` onto `first`: first's end becomes second's end, the two
    /// transcriptions are joined with a space as an ordinary collaborative
    /// edit of first's `content`, and second is tombstoned (its words stay
    /// with it, restorable). Refused unless both are on one tier and second
    /// is the very next segment after first in time order. One resolving txn.
    pub fn merge_segments(&mut self, text: &str, first: &str, second: &str) -> Result<CommitResult, String> {
        self.require_in_text(first, "segment", text)?;
        self.require_in_text(second, "segment", text)?;
        if first == second {
            return Err("a segment cannot be merged with itself".into());
        }
        let tier = self.proj.field_str(first, "tier")?.unwrap_or_default();
        if self.proj.field_str(second, "tier")?.as_deref() != Some(tier.as_str()) {
            return Err("only segments on the same tier can be merged".into());
        }
        let order = self.tier_spans(text, &tier)?;
        let at = order.iter().position(|(id, _, _)| id == first).ok_or_else(|| format!("segment {first} has no times"))?;
        if order.get(at + 1).map(|(id, _, _)| id.as_str()) != Some(second) {
            return Err("only a segment and the one right after it can be merged".into());
        }
        let (first_end, second_end) = (order[at].2, order[at + 1].2);
        let a = self.proj.field_str(first, "content")?.unwrap_or_default();
        let b = self.proj.field_str(second, "content")?.unwrap_or_default();
        let joined = match (a.trim().is_empty(), b.trim().is_empty()) {
            (_, true) => a.clone(),
            (true, false) => b.clone(),
            (false, false) => format!("{} {}", a.trim_end(), b.trim_start()),
        };
        let mut t = self.intent("merge-segments").resolving();
        if second_end > first_end {
            t.set("segment", first, text, "end", Value::Num(second_end))?;
        }
        if joined != a {
            t.text_edit("segment", first, text, "content", &joined)?;
        }
        t.delete("segment", second, text);
        t.commit()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn project() -> (PathBuf, Project, String, String) {
        let root = std::env::temp_dir().join(format!("wugs-core-segments-{}", wugs_model::ulid()));
        let mut p = Project::open(&root, "prs_Alice").unwrap();
        let txt = p.create_text("t").unwrap();
        let (tier, _) = p.add_tier("utterance", "time-aligned", None, None).unwrap();
        (root, p, txt, tier)
    }

    fn row(p: &Project, seg: &str) -> (f64, f64, String, bool) {
        let (s, e) = span(p, seg).unwrap().unwrap();
        (s, e, p.proj.field_str(seg, "content").unwrap().unwrap_or_default(), p.proj.is_tombstoned(seg).unwrap())
    }

    #[test]
    fn split_then_merge_round_trips_and_survives_replay() {
        let (root, mut p, txt, tier) = project();
        let (a, _) = p.add_segment(&txt, &tier, 0, Some(0.0), Some(2000.0), "one two").unwrap();
        let (c, _) = p.add_segment(&txt, &tier, 1, Some(3000.0), Some(4000.0), "three").unwrap();
        assert!(p.split_segment(&txt, &a, 0.0).is_err(), "a cut on an edge is refused");
        assert!(p.split_segment(&txt, &a, 2500.0).is_err(), "a cut outside is refused");
        let (b, _) = p.split_segment(&txt, &a, 1200.0).unwrap();
        assert_eq!(row(&p, &a), (0.0, 1200.0, "one two".into(), false));
        assert_eq!(row(&p, &b), (1200.0, 2000.0, String::new(), false));
        assert_eq!(p.proj.ordered_children("text", &txt, "segment").unwrap(), [a.clone(), b.clone(), c.clone()], "the new part sits right after");

        assert!(p.merge_segments(&txt, &a, &c).is_err(), "c is not next after a");
        p.merge_segments(&txt, &a, &b).unwrap();
        assert_eq!(row(&p, &a), (0.0, 2000.0, "one two".into(), false));
        assert!(p.proj.is_tombstoned(&b).unwrap());

        // The undo of the merge: restore, retime, edit back.
        p.restore_segment(&txt, &b).unwrap();
        p.set_segment_times(&txt, &a, None, Some(1200.0)).unwrap();
        assert_eq!(row(&p, &b), (1200.0, 2000.0, String::new(), false));
        assert!(p.restore_segment(&txt, &b).is_err(), "only a deleted segment is restored");

        p.replay().unwrap();
        assert_eq!(row(&p, &a), (0.0, 1200.0, "one two".into(), false));
        assert_eq!(row(&p, &b), (1200.0, 2000.0, String::new(), false));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn merge_joins_text_as_an_edit_and_refuses_other_tiers() {
        let (root, mut p, txt, tier) = project();
        let (other, _) = p.add_tier("notes", "time-aligned", None, None).unwrap();
        let (a, _) = p.add_segment(&txt, &tier, 0, Some(0.0), Some(1000.0), "köpek").unwrap();
        let (b, _) = p.add_segment(&txt, &tier, 1, Some(1100.0), Some(2000.0), "geldi").unwrap();
        let (n, _) = p.add_segment(&txt, &other, 2, Some(1000.0), Some(1050.0), "x").unwrap();
        assert!(p.merge_segments(&txt, &a, &n).unwrap_err().contains("same tier"));
        assert!(p.merge_segments(&txt, &b, &a).is_err(), "order matters");
        p.merge_segments(&txt, &a, &b).unwrap();
        assert_eq!(row(&p, &a), (0.0, 2000.0, "köpek geldi".into(), false));
        // An ordinary edit afterwards still merges (the text history is kept).
        p.edit_content(&txt, &a, "köpek gitti").unwrap();
        assert_eq!(row(&p, &a).2, "köpek gitti");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn machine_segments_skip_what_a_person_drew_and_are_one_txn() {
        let (root, mut p, txt, tier) = project();
        let (mine, _) = p.add_segment(&txt, &tier, 0, Some(900.0), Some(1500.0), "drawn").unwrap();
        let spans = [(0.0, 800.0), (1000.0, 2000.0), (2500.0, 3000.0), (2900.0, 3500.0), (4000.0, 3900.0)];
        let (ids, res) = p.add_segments(&txt, &tier, 1, &spans).unwrap();
        assert_eq!(ids.len(), 2, "the overlap with the drawn one, the overlap within the call and the backwards span are skipped");
        assert_eq!(res.ops, 2);
        assert_eq!(row(&p, &ids[0]), (0.0, 800.0, String::new(), false));
        assert_eq!(row(&p, &ids[1]), (2500.0, 3000.0, String::new(), false));
        assert_eq!(row(&p, &mine).2, "drawn");
        let txns: std::collections::BTreeSet<String> = p.log.read_all().unwrap().ops.into_iter().filter(|o| o.intent.as_deref() == Some(MACHINE_SEGMENT_INTENT)).map(|o| o.txn).collect();
        assert_eq!(txns.len(), 1);
        // Undo deletes them all in one txn; redo brings the same ids back.
        assert_eq!(p.delete_segments(&txt, &ids).unwrap().ops, 2);
        assert!(p.delete_segments(&txt, &ids).is_err(), "already deleted");
        assert_eq!(p.restore_segments(&txt, &ids).unwrap().ops, 2);
        assert_eq!(row(&p, &ids[1]), (2500.0, 3000.0, String::new(), false));
        p.delete_segments(&txt, &ids).unwrap();
        let (again, res) = p.add_segments(&txt, &tier, 3, &[(1000.0, 1400.0)]).unwrap();
        assert!(again.is_empty() && res.ops == 0, "nothing to write is no txn");
        std::fs::remove_dir_all(root).ok();
    }
}
