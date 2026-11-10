//! A round trip through the user's own Praat (roadmap 46).
//!
//! [`hand_out`] writes a text as a TextGrid plus a manifest naming the
//! segment behind every interval and point. After the linguist edits the
//! grid in Praat, [`plan_return`] compares the edited file with the grid that
//! went out (never with the project, which may have moved on), so only what
//! changed in Praat becomes a change: a moved boundary, an edited label, a
//! new labelled interval or point, a removed one. [`apply_return`] writes the
//! plan as one resolving txn, because every change is a person's edit.
//!
//! Matching needs no ids inside the file. Intervals tile their tier, so each
//! edited interval belongs to an exported one it overlaps (preferring a
//! segment over a gap, then an unchanged label, then the most overlap), and
//! each exported interval keeps the edited one that ranks best. Splitting an interval in Praat leaves the
//! segment on the part that kept its label; merging two keeps the first and
//! removes the second; clearing an interval's label removes its segment,
//! since an empty interval is Praat's only way to say "nothing here" (a
//! point can be deleted outright, so clearing its mark is a relabel). A
//! segment always outranks a gap, so an edge dragged far into the gap
//! beside it stays with its segment. Points match the nearest exported point within half a
//! second. Tiers match by name; tiers added in Praat are reported and not
//! brought back, and a tier missing from the edited file deletes nothing.

use std::collections::HashMap;

use wugs_core::{CommitResult, Project};
use wugs_model::{json, Value};

use crate::common::{decode, opt_str, Counts};
use crate::textgrid::{export_mapped, parse, Class, Item, TierMap};

/// Points further apart than this are different points.
const POINT_REACH_S: f64 = 0.5;

/// What goes to Praat: the grid, and the manifest to read the edited grid
/// back against.
pub struct Handoff {
    pub textgrid: String,
    pub manifest: String,
    /// Segments the grid cannot hold (overlaps on a tier and the like), by
    /// reason. They are not in the grid, so Praat cannot change them.
    pub not_exported: Counts,
}

pub fn hand_out(project: &Project, text: &str) -> Result<Handoff, String> {
    let (export, maps) = export_mapped(project, text)?;
    let tiers = maps
        .iter()
        .map(|m| {
            Value::obj(vec![
                ("tier", Value::str(&m.tier)),
                ("name", Value::str(&m.name)),
                ("items", Value::Arr(m.items.iter().map(|i| i.as_deref().map_or(Value::Null, Value::str)).collect())),
            ])
        })
        .collect();
    let manifest = Value::obj(vec![("text", Value::str(text)), ("tiers", Value::Arr(tiers))]).to_json();
    Ok(Handoff { textgrid: export.textgrid, manifest, not_exported: export.not_exported })
}

#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// Only the edges that moved are given.
    Retime { segment: String, start_ms: Option<f64>, end_ms: Option<f64> },
    Relabel { segment: String, label: String },
    Add { tier: String, start_ms: f64, end_ms: f64, label: String },
    Remove { segment: String },
}

#[derive(Debug, Default)]
pub struct Plan {
    pub changes: Vec<Change>,
    /// What Praat changed that is not brought back, by reason.
    pub skipped: Counts,
    /// The grid as Praat saved it, and what each of its items will be once
    /// the plan is applied: the baseline for the next return
    /// ([`Plan::rebased`]).
    edited: String,
    after: Vec<TierAfter>,
    text: String,
}

#[derive(Debug)]
struct TierAfter {
    /// Empty for a tier wugs does not take back (added in Praat, or whose
    /// type changed).
    tier: String,
    name: String,
    items: Vec<Slot>,
}

#[derive(Clone, Debug)]
enum Slot {
    Gap,
    Seg(String),
    /// The n-th `Change::Add` of the plan.
    Added(usize),
}

/// What [`apply_return`] wrote.
pub struct Applied {
    pub commit: Option<CommitResult>,
    /// The new segments, one per `Change::Add` in plan order.
    pub created: Vec<String>,
}

#[derive(Debug, Default, PartialEq)]
pub struct PlanCounts {
    pub retimed: usize,
    pub relabelled: usize,
    pub added: usize,
    pub removed: usize,
}

impl Plan {
    /// The next baseline: the grid Praat saved and a manifest naming the
    /// segment behind each item after `applied`. Comparing a later save
    /// with this, and not with a fresh export, keeps changes made in Praat
    /// from coming back twice and keeps edits made elsewhere meanwhile from
    /// looking like deletions in Praat.
    pub fn rebased(&self, applied: &Applied) -> Result<(String, String), String> {
        let mut tiers = Vec::new();
        for t in &self.after {
            let mut items = Vec::new();
            for slot in &t.items {
                items.push(match slot {
                    Slot::Gap => Value::Null,
                    Slot::Seg(id) => Value::str(id),
                    Slot::Added(k) => Value::str(applied.created.get(*k).ok_or("the plan was not applied")?),
                });
            }
            tiers.push(Value::obj(vec![("tier", Value::str(&t.tier)), ("name", Value::str(&t.name)), ("items", Value::Arr(items))]));
        }
        let text = self.text.clone();
        let manifest = Value::obj(vec![("text", Value::str(&text)), ("tiers", Value::Arr(tiers))]).to_json();
        Ok((self.edited.clone(), manifest))
    }

    pub fn counts(&self) -> PlanCounts {
        let mut c = PlanCounts::default();
        for ch in &self.changes {
            match ch {
                Change::Retime { .. } => c.retimed += 1,
                Change::Relabel { .. } => c.relabelled += 1,
                Change::Add { .. } => c.added += 1,
                Change::Remove { .. } => c.removed += 1,
            }
        }
        c
    }
}

fn read_manifest(manifest: &str) -> Result<(String, Vec<TierMap>), String> {
    let bad = || "the handoff manifest is damaged".to_string();
    let v = json::parse(manifest).map_err(|_| bad())?;
    let text = v.get_str("text").ok_or_else(bad)?.to_string();
    let Some(Value::Arr(tiers)) = v.get("tiers") else {
        return Err(bad());
    };
    let mut maps = Vec::new();
    for t in tiers {
        let Some(Value::Arr(items)) = t.get("items") else {
            return Err(bad());
        };
        maps.push(TierMap {
            tier: t.get_str("tier").ok_or_else(bad)?.to_string(),
            name: t.get_str("name").ok_or_else(bad)?.to_string(),
            items: items.iter().map(|i| i.as_str().map(str::to_string)).collect(),
        });
    }
    Ok((text, maps))
}

/// Compare the edited grid with the one that went out. `edited` is the file
/// as Praat saved it (UTF-8, UTF-16 or Latin-1).
pub fn plan_return(project: &Project, text: &str, baseline: &str, manifest: &str, edited: &[u8]) -> Result<Plan, String> {
    let (for_text, maps) = read_manifest(manifest)?;
    if for_text != text {
        return Err("this handoff belongs to another text".into());
    }
    let base = parse(baseline).map_err(|e| format!("the exported grid no longer reads: {e}"))?;
    let edited_text = decode(edited).map_err(|e| format!("Praat's TextGrid could not be read: {e}"))?;
    let edited = parse(&edited_text).map_err(|e| format!("Praat's TextGrid could not be read: {e}"))?;
    if base.tiers.len() != maps.len() || base.tiers.iter().zip(&maps).any(|(t, m)| t.items.len() != m.items.len()) {
        return Err("the handoff files do not match each other".into());
    }

    let mut plan = Plan { edited: edited_text.clone(), text: text.to_string(), ..Plan::default() };
    plan.after = edited.tiers.iter().map(|t| TierAfter { tier: String::new(), name: t.name.clone(), items: vec![Slot::Gap; t.items.len()] }).collect();
    let mut adds = 0usize;
    let mut taken = vec![false; edited.tiers.len()];
    // Live segments by (tier, start, end, label), so an add that is already
    // in wugs (applied before a crash kept the baseline from moving on) is
    // not added twice.
    let mut present: HashMap<(String, i64, i64, String), String> = HashMap::new();
    for seg in project.proj.children_by_ref("text", text, "segment")? {
        if project.proj.is_tombstoned(&seg)? {
            continue;
        }
        let (Some(st), Some(en)) = (project.proj.field_num(&seg, "start")?, project.proj.field_num(&seg, "end")?) else {
            continue;
        };
        let tier = opt_str(project, &seg, "tier")?.unwrap_or_default();
        let label = opt_str(project, &seg, "content")?.unwrap_or_default();
        present.insert((tier, st.round() as i64, en.round() as i64, label), seg);
    }
    let mut seen_names: HashMap<&str, usize> = HashMap::new();
    for (bt, map) in base.tiers.iter().zip(&maps) {
        // The n-th tier of a name matches the n-th edited tier of that name.
        let nth = seen_names.entry(bt.name.as_str()).or_default();
        let found = edited.tiers.iter().enumerate().filter(|(_, t)| t.name == bt.name).nth(*nth).map(|(i, _)| i);
        *nth += 1;
        let Some(ei) = found else {
            plan.skipped.add("tier missing from Praat's grid (nothing deleted)");
            continue;
        };
        taken[ei] = true;
        let et = &edited.tiers[ei];
        if map.tier.is_empty() {
            // Added in Praat at an earlier return: still not taken back.
            if et.items.iter().any(|it| !it.label.is_empty()) {
                plan.skipped.add("tier added in Praat");
            }
            continue;
        }
        if et.class != bt.class {
            plan.skipped.add("tier whose type changed in Praat");
            continue;
        }
        plan.after[ei].tier = map.tier.clone();
        let point = bt.class == Class::Text;
        let (for_base, for_edit) = if point { match_points(&bt.items, &et.items) } else { match_intervals(&bt.items, &et.items, &map.items.iter().map(Option::is_some).collect::<Vec<_>>()) };

        for (i, b) in bt.items.iter().enumerate() {
            let kept = for_base[i];
            match (&map.items[i], kept.map(|j| &et.items[j])) {
                (Some(seg), _) if !live_in(project, seg, text)? => {
                    plan.skipped.add("segment deleted in wugs since it went to Praat");
                    // Still in Praat's grid: keep it named in the next
                    // baseline, or the next return would add it again.
                    if let Some(j) = kept {
                        plan.after[ei].items[j] = Slot::Seg(seg.clone());
                    }
                }
                // An empty interval is how Praat says "nothing here", so a
                // label cleared in Praat removes the segment.
                // Points can be deleted outright in Praat, so a cleared mark
                // is only a relabel there.
                (Some(seg), Some(e)) if !point && e.label.is_empty() && !b.label.is_empty() => {
                    plan.changes.push(Change::Remove { segment: seg.clone() })
                }
                (Some(seg), Some(e)) => {
                    if let Some(j) = kept {
                        plan.after[ei].items[j] = Slot::Seg(seg.clone());
                    }
                    let moved = |a: &crate::textgrid::Time, b: &crate::textgrid::Time| (a.ms() != b.ms()).then(|| b.ms());
                    let (start_ms, end_ms) = if point {
                        let m = moved(&b.start, &e.start);
                        (m, m)
                    } else {
                        (moved(&b.start, &e.start), moved(&b.end, &e.end))
                    };
                    if start_ms.is_some() || end_ms.is_some() {
                        plan.changes.push(Change::Retime { segment: seg.clone(), start_ms, end_ms });
                    }
                    if e.label != b.label {
                        plan.changes.push(Change::Relabel { segment: seg.clone(), label: e.label.clone() });
                    }
                }
                (Some(seg), None) => plan.changes.push(Change::Remove { segment: seg.clone() }),
                (None, _) => {}
            }
        }
        // Labelled items with no segment behind them: typed into a gap, or
        // new in Praat.
        for (j, e) in et.items.iter().enumerate() {
            // Behind it only if the segment it chose also kept it: the
            // unlabelled-to-labelled half of a split chose a segment that
            // kept the other half.
            let segment_behind = for_edit[j].is_some_and(|i| map.items[i].is_some() && for_base[i] == Some(j));
            if segment_behind || (e.label.is_empty() && !point) {
                continue;
            }
            if !point && e.end.ms() <= e.start.ms() {
                plan.skipped.add("interval shorter than a millisecond");
                continue;
            }
            if let Some(seg) = present.get(&(map.tier.clone(), e.start.ms() as i64, e.end.ms() as i64, e.label.clone())) {
                plan.skipped.add("already in wugs");
                plan.after[ei].items[j] = Slot::Seg(seg.clone());
                continue;
            }
            plan.changes.push(Change::Add { tier: map.tier.clone(), start_ms: e.start.ms(), end_ms: e.end.ms(), label: e.label.clone() });
            plan.after[ei].items[j] = Slot::Added(adds);
            adds += 1;
        }
    }
    for (i, t) in edited.tiers.iter().enumerate() {
        if !taken[i] && t.items.iter().any(|it| !it.label.is_empty()) {
            plan.skipped.add("tier added in Praat");
        }
    }
    Ok(plan)
}

fn live_in(project: &Project, seg: &str, text: &str) -> Result<bool, String> {
    Ok(project.proj.entity_kind(seg)?.as_deref() == Some("segment")
        && !project.proj.is_tombstoned(seg)?
        && project.proj.field_str(seg, "text")?.as_deref() == Some(text))
}

/// For each base item the edited item kept for it, and for each edited item
/// the base item it chose (which may have kept another).
type Matching = (Vec<Option<usize>>, Vec<Option<usize>>);

/// `segment[i]`: whether base item i has a segment behind it (not a gap).
fn match_intervals(base: &[Item], edited: &[Item], segment: &[bool]) -> Matching {
    // In whole ms, the unit wugs stores, so equal halves of a split tie
    // (2.3 - 2.0 and 2.6 - 2.3 differ in binary floating point).
    let overlap = |a: &Item, b: &Item| (a.end.secs.min(b.end.secs) - a.start.secs.max(b.start.secs)) * 1000.0;
    let overlap = |a: &Item, b: &Item| overlap(a, b).round();
    // Ranks a pairing: a segment before a gap (an edge dragged far into the
    // gap beside it still belongs to its segment), then an unchanged label
    // (the shared boundary of two segments moved past the middle), then
    // more overlap; the earlier item wins a full tie.
    let rank = |i: usize, j: usize| (segment[i], base[i].label == edited[j].label, overlap(&base[i], &edited[j]));
    let better = |r: (bool, bool, f64), best: Option<(bool, bool, f64)>| best.is_none_or(|b| r.partial_cmp(&b) == Some(std::cmp::Ordering::Greater));
    let mut chose = vec![None; edited.len()];
    let mut lo = 0;
    for (j, e) in edited.iter().enumerate() {
        while lo < base.len() && base[lo].end.secs <= e.start.secs {
            lo += 1;
        }
        let mut best: Option<(usize, (bool, bool, f64))> = None;
        for (i, b) in base.iter().enumerate().skip(lo) {
            if b.start.secs >= e.end.secs {
                break;
            }
            let r = rank(i, j);
            if r.2 > 0.0 && better(r, best.map(|(_, r)| r)) {
                best = Some((i, r));
            }
        }
        chose[j] = best.map(|(i, _)| i);
    }
    let mut kept: Vec<Option<(usize, (bool, bool, f64))>> = vec![None; base.len()];
    for (j, c) in chose.iter().enumerate() {
        if let Some(i) = *c {
            let r = rank(i, j);
            if better(r, kept[i].map(|(_, r)| r)) {
                kept[i] = Some((j, r));
            }
        }
    }
    (kept.into_iter().map(|k| k.map(|(j, _)| j)).collect(), chose)
}

fn match_points(base: &[Item], edited: &[Item]) -> Matching {
    let dist = |a: &Item, b: &Item| (a.start.secs - b.start.secs).abs();
    let mut chose = vec![None; edited.len()];
    for (j, e) in edited.iter().enumerate() {
        let mut best: Option<(usize, f64, bool)> = None;
        for (i, b) in base.iter().enumerate() {
            let d = dist(b, e);
            let same = b.label == e.label;
            if d <= POINT_REACH_S && best.is_none_or(|(_, bd, bs)| d < bd || (d == bd && same && !bs)) {
                best = Some((i, d, same));
            }
        }
        chose[j] = best.map(|(i, _, _)| i);
    }
    let mut kept: Vec<Option<(usize, f64)>> = vec![None; base.len()];
    for (j, c) in chose.iter().enumerate() {
        if let Some(i) = *c {
            let d = dist(&base[i], &edited[j]);
            if kept[i].is_none_or(|(_, kd)| d < kd) {
                kept[i] = Some((j, d));
            }
        }
    }
    (kept.into_iter().map(|k| k.map(|(j, _)| j)).collect(), chose)
}

/// Write a plan as one resolving txn. Returns None when there is nothing to
/// write. Run [`plan_return`] right before, so the plan is current.
pub fn apply_return(project: &mut Project, text: &str, plan: &Plan) -> Result<Applied, String> {
    if plan.changes.is_empty() {
        return Ok(Applied { commit: None, created: Vec::new() });
    }
    let mut created = Vec::new();
    let mut next = 0usize;
    for seg in project.proj.children_by_ref("text", text, "segment")? {
        let pos = opt_str(project, &seg, "pos")?.unwrap_or_default();
        next = next.max(usize::from_str_radix(&pos, 16).map_or(1, |i| i + 1));
    }
    let mut t = project.intent("praat-return").resolving();
    for ch in &plan.changes {
        match ch {
            Change::Retime { segment, start_ms, end_ms } => {
                if let Some(s) = start_ms {
                    t.set("segment", segment, text, "start", Value::Num(*s))?;
                }
                if let Some(e) = end_ms {
                    t.set("segment", segment, text, "end", Value::Num(*e))?;
                }
            }
            Change::Relabel { segment, label } => t.text_edit("segment", segment, text, "content", label)?,
            Change::Add { tier, start_ms, end_ms, label } => {
                created.push(t.create(
                    "segment",
                    "seg",
                    text,
                    vec![
                        ("text", Value::str(text)),
                        ("tier", Value::str(tier)),
                        ("pos", Value::str(&wugs_model::pos_seq(next))),
                        ("content", Value::str(label)),
                        ("start", Value::Num(*start_ms)),
                        ("end", Value::Num(*end_ms)),
                    ],
                ));
                next += 1;
            }
            Change::Remove { segment } => t.delete("segment", segment, text),
        }
    }
    let commit = t.commit()?;
    Ok(Applied { commit: Some(commit), created })
}
