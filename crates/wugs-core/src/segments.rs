//! Timeline cutting (roadmap 59): machine segmentation, split, merge,
//! unmerge and restore. Each is one txn, so it lands whole or not at all,
//! and each is an ordinary write: an undo in the shell is the inverse
//! written as a new edit (a split is undone by a merge, a merge by an
//! unmerge, a delete by a restore), never a rewrite of the log.
//!
//! An undo must never lose work done since the edit it takes back (in the
//! Transcribe or Gloss panel, or on another device). So the shell keeps a
//! [`SegmentState`] of what each undo would delete or overwrite, and the
//! undo first checks it with [`Project::require_unchanged`] (or deletes
//! only the untouched segments, [`Project::delete_unchanged_segments`]).

use wugs_model::{pos_between, pos_seq, Value};

use crate::{CommitResult, Project};

/// The intent machine-proposed segments are written under, so the log says
/// which boundaries a person drew and which a segmenter proposed.
pub const MACHINE_SEGMENT_INTENT: &str = "machine-segment";

/// What a timeline undo or redo would delete or overwrite on one segment:
/// its times, transcription, how many words it has, and whether it is
/// deleted. Compared, never written.
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentState {
    pub id: String,
    pub deleted: bool,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub content: String,
    /// Live words (tokens) in its `members` view.
    pub words: usize,
}

/// The parts of a [`SegmentState`] an undo depends on; `None` is not
/// checked (a retime undo checks only the edge it writes back).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SegmentCheck {
    pub id: String,
    pub deleted: Option<bool>,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub content: Option<String>,
    pub words: Option<usize>,
}

impl SegmentCheck {
    /// Every part: for an undo that deletes the segment.
    pub fn all(s: &SegmentState) -> SegmentCheck {
        SegmentCheck {
            id: s.id.clone(),
            deleted: Some(s.deleted),
            start: s.start,
            end: s.end,
            content: Some(s.content.clone()),
            words: Some(s.words),
        }
    }

    pub fn holds(&self, s: &SegmentState) -> bool {
        self.deleted.is_none_or(|d| d == s.deleted)
            && self.start.is_none_or(|v| Some(v) == s.start)
            && self.end.is_none_or(|v| Some(v) == s.end)
            && self.content.as_ref().is_none_or(|c| *c == s.content)
            && self.words.is_none_or(|w| w == s.words)
    }
}

/// A segment's time span, when it has both edges.
fn span(p: &Project, seg: &str) -> Result<Option<(f64, f64)>, String> {
    Ok(match (p.proj.field_num(seg, "start")?, p.proj.field_num(seg, "end")?) {
        (Some(s), Some(e)) => Some((s, e)),
        _ => None,
    })
}

impl Project {
    /// What an undo checks on `seg` (deleted or not).
    pub fn segment_state(&self, seg: &str) -> Result<SegmentState, String> {
        if self.proj.entity_kind(seg)?.as_deref() != Some("segment") {
            return Err(format!("no such segment: {seg}"));
        }
        Ok(SegmentState {
            id: seg.to_string(),
            deleted: self.proj.is_tombstoned(seg)?,
            start: self.proj.field_num(seg, "start")?,
            end: self.proj.field_num(seg, "end")?,
            content: self.proj.field_str(seg, "content")?.unwrap_or_default(),
            words: self.proj.members(seg)?.len(),
        })
    }

    pub fn segment_states(&self, segs: &[String]) -> Result<Vec<SegmentState>, String> {
        segs.iter().map(|s| self.segment_state(s)).collect()
    }

    /// Refuse, writing nothing, when any check no longer holds (the segment
    /// was edited since, anywhere) or a field it checks has an open
    /// conflict (an undo is a direct edit and would settle it unseen).
    pub fn require_unchanged(&self, checks: &[SegmentCheck]) -> Result<(), String> {
        let mut changed = 0;
        for c in checks {
            if !c.holds(&self.segment_state(&c.id)?) {
                changed += 1;
            }
        }
        if changed > 0 {
            return Err(if checks.len() == 1 {
                "That segment was edited since (here, in another panel or on another device), so this step was skipped to keep that work.".to_string()
            } else {
                format!("{changed} of the segments were edited since (here, in another panel or on another device), so this step was skipped to keep that work.")
            });
        }
        for c in checks {
            for (field, checked) in [("start", c.start.is_some()), ("end", c.end.is_some()), ("content", c.content.is_some())] {
                if checked && self.proj.has_conflict(&c.id, field)? {
                    return Err("A segment this step would change has a conflict to settle first (Conflicts panel), so this step was skipped.".into());
                }
            }
        }
        Ok(())
    }

    /// Delete, in one txn, the segments whose checks still hold, and keep
    /// the rest (edited since: deleting them would lose that work). The
    /// undo of an Auto-segment or a drawn segment. Returns (deleted, kept);
    /// nothing is written when every one was kept.
    pub fn delete_unchanged_segments(&mut self, text: &str, checks: &[SegmentCheck]) -> Result<(Vec<String>, Vec<String>, CommitResult), String> {
        let (mut gone, mut kept) = (Vec::new(), Vec::new());
        for c in checks {
            self.require_in_text(&c.id, "segment", text)?;
            if c.holds(&self.segment_state(&c.id)?) {
                gone.push(c.id.clone());
            } else {
                kept.push(c.id.clone());
            }
        }
        let res = self.delete_segments(text, &gone)?;
        Ok((gone, kept, res))
    }

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
    /// right after it in `pos` order (deleted segments count, so a restore
    /// never brings back a duplicate position), with the same `parent` and
    /// `participant`. The transcription and its words stay on the first
    /// part; the new part starts empty, because no rule can tell which
    /// words were said after the cut. One resolving txn (a person's direct
    /// edit, like a retime). Returns the new segment's id.
    pub fn split_segment(&mut self, text: &str, segment: &str, at_ms: f64) -> Result<(String, CommitResult), String> {
        self.require_in_text(segment, "segment", text)?;
        let (start, end) = span(self, segment)?.ok_or_else(|| format!("segment {segment} has no times to split"))?;
        if !(at_ms.is_finite() && at_ms > start && at_ms < end) {
            return Err(format!("{at_ms} ms is not inside the segment ({start} to {end} ms)"));
        }
        let tier = self.proj.field_str(segment, "tier")?.unwrap_or_default();
        let pos = self.proj.field_str(segment, "pos")?.unwrap_or_default();
        let mut next: Option<String> = None;
        for sib in self.proj.children_by_ref_with_deleted("text", text, "segment")? {
            let p = self.proj.field_str(&sib, "pos")?.unwrap_or_default();
            if p > pos && next.as_ref().is_none_or(|n| p < *n) {
                next = Some(p);
            }
        }
        let new_pos = pos_between(&pos, next.as_deref());
        let mut fields = vec![
            ("text", Value::str(text)),
            ("tier", Value::str(&tier)),
            ("pos", Value::str(&new_pos)),
            ("content", Value::str("")),
            ("start", Value::Num(at_ms)),
            ("end", Value::Num(end)),
        ];
        for kept in ["parent", "participant"] {
            if let Some((v, _)) = self.proj.field(segment, kept)? {
                fields.push((kept, v));
            }
        }
        let mut t = self.intent("split-segment").resolving();
        t.set("segment", segment, text, "end", Value::Num(at_ms))?;
        let id = t.create("segment", "seg", text, fields);
        let res = t.commit()?;
        Ok((id, res))
    }

    /// Join `second` onto `first`: first's end becomes second's end, the two
    /// transcriptions are joined with a space as an ordinary collaborative
    /// edit of first's `content`, and second is tombstoned (its words stay
    /// with it, restorable). Refused unless both are on one tier and second
    /// is the very next segment after first in time order, and while either
    /// transcription has an open conflict (the merge is a direct edit and
    /// would settle it without anyone seeing the other version). One
    /// resolving txn.
    pub fn merge_segments(&mut self, text: &str, first: &str, second: &str) -> Result<CommitResult, String> {
        self.require_in_text(first, "segment", text)?;
        self.require_in_text(second, "segment", text)?;
        if first == second {
            return Err("a segment cannot be merged with itself".into());
        }
        if self.proj.has_conflict(first, "content")? || self.proj.has_conflict(second, "content")? {
            return Err("One of these transcriptions has a conflict to settle first (Conflicts panel).".into());
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

    /// Take a merge back in one resolving txn: `second` (deleted by the
    /// merge) is restored with everything it had, `first`'s end goes back
    /// to `first_end`, and, when given, its transcription back to
    /// `first_content` (as a collaborative edit). Also the redo of a split:
    /// the parts come apart again at `first_end`, with no text change. The
    /// shell checks first ([`Project::require_unchanged`]) that nothing was
    /// edited since.
    pub fn unmerge_segments(&mut self, text: &str, first: &str, second: &str, first_end: f64, first_content: Option<&str>) -> Result<CommitResult, String> {
        self.require_in_text(first, "segment", text)?;
        if first == second {
            return Err("a segment cannot be unmerged from itself".into());
        }
        if self.proj.entity_kind(second)?.as_deref() != Some("segment") || self.proj.field_str(second, "text")?.as_deref() != Some(text) {
            return Err(format!("segment {second} does not belong to text {text}"));
        }
        if !self.proj.is_tombstoned(second)? {
            return Err(format!("segment {second} is not merged away (it is not deleted)"));
        }
        if self.proj.field_str(first, "tier")? != self.proj.field_str(second, "tier")? {
            return Err("only segments on the same tier can be unmerged".into());
        }
        let start = self.proj.field_num(first, "start")?.unwrap_or(0.0);
        if !(first_end.is_finite() && first_end > start) {
            return Err(format!("{first_end} ms is not after the segment's start ({start} ms)"));
        }
        let retime = self.proj.field_num(first, "end")? != Some(first_end);
        let edit = match first_content {
            Some(c) if self.proj.field_str(first, "content")?.unwrap_or_default() != c => Some(c),
            _ => None,
        };
        let mut t = self.intent("unmerge-segments").resolving();
        t.restore("segment", second, text);
        if retime {
            t.set("segment", first, text, "end", Value::Num(first_end))?;
        }
        if let Some(c) = edit {
            t.text_edit("segment", first, text, "content", c)?;
        }
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

        // The undo of the merge: one txn restores b and moves a's end back.
        assert!(p.unmerge_segments(&txt, &a, &b, 0.0, None).is_err(), "an end at the start is refused");
        assert!(p.proj.is_tombstoned(&b).unwrap(), "a refused unmerge writes nothing");
        assert_eq!(p.unmerge_segments(&txt, &a, &b, 1200.0, Some("one two")).unwrap().ops, 2, "a restore and a retime; the text was unchanged");
        assert_eq!(row(&p, &b), (1200.0, 2000.0, String::new(), false));
        assert!(p.unmerge_segments(&txt, &a, &b, 1200.0, None).is_err(), "b is no longer merged away");
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

    fn txns_of(p: &Project, intent: &str) -> std::collections::BTreeSet<String> {
        p.log.read_all().unwrap().ops.into_iter().filter(|o| o.intent.as_deref() == Some(intent)).map(|o| o.txn).collect()
    }

    #[test]
    fn unmerge_is_one_txn_that_puts_both_parts_back() {
        let (root, mut p, txt, tier) = project();
        let (a, _) = p.add_segment(&txt, &tier, 0, Some(0.0), Some(1000.0), "köpek").unwrap();
        let (b, _) = p.add_segment(&txt, &tier, 1, Some(1100.0), Some(2000.0), "geldi").unwrap();
        p.merge_segments(&txt, &a, &b).unwrap();
        let merged = p.segment_state(&a).unwrap();
        assert_eq!((merged.end, merged.content.as_str()), (Some(2000.0), "köpek geldi"));
        let res = p.unmerge_segments(&txt, &a, &b, 1000.0, Some("köpek")).unwrap();
        assert_eq!(res.ops, 3, "restore, retime and text edit");
        assert_eq!(txns_of(&p, "unmerge-segments").len(), 1, "one txn");
        assert_eq!(row(&p, &a), (0.0, 1000.0, "köpek".into(), false));
        assert_eq!(row(&p, &b), (1100.0, 2000.0, "geldi".into(), false));
        // A failure part way (b already live) writes none of it.
        p.set_segment_times(&txt, &a, None, Some(1050.0)).unwrap();
        assert!(p.unmerge_segments(&txt, &a, &b, 1000.0, Some("x")).is_err());
        assert_eq!(row(&p, &a), (0.0, 1050.0, "köpek".into(), false));
        p.replay().unwrap();
        assert_eq!(row(&p, &b), (1100.0, 2000.0, "geldi".into(), false));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn an_undo_check_refuses_after_any_later_edit() {
        let (root, mut p, txt, tier) = project();
        let (a, _) = p.add_segment(&txt, &tier, 0, Some(0.0), Some(1000.0), "").unwrap();
        let (b, _) = p.add_segment(&txt, &tier, 1, Some(1000.0), Some(2000.0), "").unwrap();
        let checks: Vec<SegmentCheck> = p.segment_states(&[a.clone(), b.clone()]).unwrap().iter().map(SegmentCheck::all).collect();
        p.require_unchanged(&checks).unwrap();
        // Transcribed elsewhere (the Transcribe panel), then words made (Gloss).
        p.edit_content(&txt, &b, "geldi").unwrap();
        assert!(p.require_unchanged(&checks).unwrap_err().contains("edited since"));
        let (gone, kept, res) = p.delete_unchanged_segments(&txt, &checks).unwrap();
        assert_eq!((gone, kept, res.ops), (vec![a.clone()], vec![b.clone()], 1), "only the untouched one goes");
        assert_eq!(row(&p, &b), (1000.0, 2000.0, "geldi".into(), false));
        // A retime undo checks only its edge: typing does not block it.
        let edge = SegmentCheck { id: b.clone(), end: Some(2000.0), ..Default::default() };
        p.require_unchanged(std::slice::from_ref(&edge)).unwrap();
        p.tokenize(&txt, &b, &["geldi"]).unwrap();
        let st = p.segment_state(&b).unwrap();
        assert_eq!(st.words, 1);
        assert!(!SegmentCheck { words: Some(0), ..SegmentCheck::all(&st) }.holds(&st));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn merge_and_undo_checks_refuse_an_open_conflict() {
        let (ra, mut a, txt, tier) = project();
        let (rb, mut b, _, _) = project();
        let (s1, _) = a.add_segment(&txt, &tier, 0, Some(0.0), Some(1000.0), "").unwrap();
        let (s2, _) = a.add_segment(&txt, &tier, 1, Some(1000.0), Some(2000.0), "").unwrap();
        b.ingest(a.log.read_all().unwrap().ops).unwrap();
        // Both devices type into s2 offline (no text merger here: LWW plus a conflict).
        a.edit_content(&txt, &s2, "geldi").unwrap();
        b.edit_content(&txt, &s2, "gitti").unwrap();
        let theirs: Vec<_> = b.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == b.device()).collect();
        a.ingest(theirs).unwrap();
        assert!(a.proj.has_conflict(&s2, "content").unwrap());
        assert!(a.merge_segments(&txt, &s1, &s2).unwrap_err().contains("Conflicts panel"));
        assert!(!a.proj.is_tombstoned(&s2).unwrap());
        let st = a.segment_state(&s2).unwrap();
        assert!(a.require_unchanged(&[SegmentCheck::all(&st)]).unwrap_err().contains("Conflicts panel"));
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }

    #[test]
    fn split_keeps_parent_and_participant_and_never_reuses_a_deleted_position() {
        let (root, mut p, txt, tier) = project();
        let (a, _) = p.add_segment(&txt, &tier, 0, Some(0.0), Some(2000.0), "").unwrap();
        let (c, _) = p.add_segment(&txt, &tier, 1, Some(3000.0), Some(4000.0), "").unwrap();
        let mut t = p.intent("test");
        t.set("segment", &a, &txt, "parent", Value::str(&c)).unwrap();
        t.set("segment", &a, &txt, "participant", Value::str("prs_Alice")).unwrap();
        t.commit().unwrap();
        let (b, _) = p.split_segment(&txt, &a, 1000.0).unwrap();
        assert_eq!(p.proj.field_str(&b, "parent").unwrap().as_deref(), Some(c.as_str()));
        assert_eq!(p.proj.field_str(&b, "participant").unwrap().as_deref(), Some("prs_Alice"));
        // Delete the new part and split again: the new key must differ from
        // the deleted part's, so restoring it leaves no duplicate.
        p.delete_segments(&txt, std::slice::from_ref(&b)).unwrap();
        let (b2, _) = p.split_segment(&txt, &a, 500.0).unwrap();
        p.restore_segment(&txt, &b).unwrap();
        let pos = |id: &str| p.proj.field_str(id, "pos").unwrap().unwrap();
        assert_ne!(pos(&b), pos(&b2));
        assert_eq!(p.proj.ordered_children("text", &txt, "segment").unwrap(), [a.clone(), b2, b, c]);
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
