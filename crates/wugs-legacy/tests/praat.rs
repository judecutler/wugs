//! The Praat round trip: export, edit in Praat, plan, apply.
//!
//! `fixtures/praat-edited.TextGrid` is real Praat output (6.4, UTF-16 as
//! Praat writes it), made by running `fixtures/praat-edit.praat` on
//! `fixtures/praat-out.TextGrid`, which is this file's export of the text
//! built by `text()`. To rebuild both after an export change:
//! `WUGS_WRITE_PRAAT_FIXTURE=1 cargo test -p wugs-legacy --test praat`, then
//! `praat --run crates/wugs-legacy/tests/fixtures/praat-edit.praat` from the
//! repo root, then `praat --run crates/wugs-legacy/tests/fixtures/praat-edit-2.praat`.

use std::fs;
use std::path::PathBuf;

use wugs_core::{Project, SCOPE_CONFIG};
use wugs_legacy::praat::{apply_return, hand_out, plan_return, Change};
use wugs_legacy::export_textgrid;
use wugs_model::Value;

const OUT: &str = include_str!("fixtures/praat-out.TextGrid");
const EDITED: &[u8] = include_bytes!("fixtures/praat-edited.TextGrid");
/// A second Praat session on top of `EDITED` (`fixtures/praat-edit-2.praat`).
const EDITED_2: &[u8] = include_bytes!("fixtures/praat-edited-2.TextGrid");

fn temp_project() -> (PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("wugs-legacy-praat-{}", wugs_model::ulid()));
    let p = Project::open(&root, "prs_Tester").unwrap();
    (root, p)
}

struct Built {
    text: String,
    utt: String,
    bir: String,
    iki: String,
    uc: String,
    blank: String,
    high: String,
}

/// utt: bir 0.5–1.2, iki 2.0–2.6, üç 3.0–3.5, an empty segment 4.0–4.4;
/// tones (points): H at 0.8.
fn text(p: &mut Project) -> Built {
    let mut t = p.intent("create-tiers");
    let utt = t.create("tier", "tir", SCOPE_CONFIG, vec![("name", Value::str("utt"))]);
    let tones = t.create("tier", "tir", SCOPE_CONFIG, vec![("name", Value::str("tones"))]);
    t.commit().unwrap();
    let text = p.create_text("Praat trip").unwrap();
    let bir = p.add_segment(&text, &utt, 0, Some(500.0), Some(1200.0), "bir").unwrap().0;
    let iki = p.add_segment(&text, &utt, 1, Some(2000.0), Some(2600.0), "iki").unwrap().0;
    let uc = p.add_segment(&text, &utt, 2, Some(3000.0), Some(3500.0), "üç").unwrap().0;
    let blank = p.add_segment(&text, &utt, 3, Some(4000.0), Some(4400.0), "").unwrap().0;
    let high = p.add_segment(&text, &tones, 4, Some(800.0), Some(800.0), "H").unwrap().0;
    Built { text, utt, bir, iki, uc, blank, high }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

#[test]
fn export_matches_the_fixture_praat_edited() {
    let (root, mut p) = temp_project();
    let b = text(&mut p);
    let out = hand_out(&p, &b.text).unwrap();
    if std::env::var_os("WUGS_WRITE_PRAAT_FIXTURE").is_some() {
        fs::write(fixture("praat-out.TextGrid"), &out.textgrid).unwrap();
    }
    assert_eq!(out.textgrid, OUT, "the export changed: rebuild the fixtures (see the file header)");
    assert_eq!(out.textgrid, export_textgrid(&p, &b.text).unwrap().textgrid, "hand_out writes the ordinary export");
    fs::remove_dir_all(root).ok();
}

fn field(p: &Project, id: &str, f: &str) -> Option<Value> {
    p.proj.field(id, f).unwrap().map(|(v, _)| v)
}

#[test]
fn praats_edits_come_back_as_changes() {
    let (root, mut p) = temp_project();
    let b = text(&mut p);
    let h = hand_out(&p, &b.text).unwrap();
    let plan = plan_return(&p, &b.text, &h.textgrid, &h.manifest, EDITED).unwrap();
    let mut changes = plan.changes.clone();
    changes.sort_by_key(|c| format!("{c:?}"));
    let mut expected = vec![
        // bir's end moved from 1.2 to 1.3, and its label changed.
        Change::Retime { segment: b.bir.clone(), start_ms: None, end_ms: Some(1300.0) },
        Change::Relabel { segment: b.bir.clone(), label: "biri".into() },
        // iki split at 2.3: the segment keeps the left part with its label;
        // the right part was labelled, so it is new.
        Change::Retime { segment: b.iki.clone(), start_ms: None, end_ms: Some(2300.0) },
        Change::Add { tier: b.utt.clone(), start_ms: 2300.0, end_ms: 2600.0, label: "ki".into() },
        // üç's label was cleared: Praat's way to remove it.
        Change::Remove { segment: b.uc.clone() },
        // H moved to 0.85; L is a new point.
        Change::Retime { segment: b.high.clone(), start_ms: Some(850.0), end_ms: Some(850.0) },
        Change::Add { tier: tones_of(&p, &b), start_ms: 2000.0, end_ms: 2000.0, label: "L".into() },
        // A new label typed into a gap.
        Change::Add { tier: b.utt.clone(), start_ms: 1500.0, end_ms: 1800.0, label: "dört".into() },
    ];
    expected.sort_by_key(|c| format!("{c:?}"));
    assert_eq!(changes, expected);
    assert_eq!(plan.skipped.0.get("tier added in Praat"), Some(&1), "{:?}", plan.skipped);
    assert!(changes.iter().all(|c| !matches!(c, Change::Remove { segment } if *segment == b.blank)), "an empty segment left empty stays");

    let applied = apply_return(&mut p, &b.text, &plan).unwrap();
    assert!(applied.commit.as_ref().expect("a commit").ops > 0);
    assert_eq!(applied.created.len(), 3, "ki, dört, L");
    assert_eq!(field(&p, &b.bir, "end"), Some(Value::Num(1300.0)));
    assert_eq!(field(&p, &b.bir, "start"), Some(Value::Num(500.0)), "an edge Praat left alone is not written");
    assert_eq!(p.proj.field_str(&b.bir, "content").unwrap().as_deref(), Some("biri"));
    assert!(p.proj.is_tombstoned(&b.uc).unwrap());
    let utt_contents: Vec<String> = p
        .proj
        .children_by_ref("text", &b.text, "segment")
        .unwrap()
        .into_iter()
        .filter(|s| !p.proj.is_tombstoned(s).unwrap())
        .filter_map(|s| p.proj.field_str(&s, "content").unwrap())
        .collect();
    for want in ["ki", "dört", "L"] {
        assert!(utt_contents.iter().any(|c| c == want), "{want} added: {utt_contents:?}");
    }

    // A fresh export matches what Praat had: nothing to bring back.
    let again = hand_out(&p, &b.text).unwrap();
    let edited_now = export_textgrid(&p, &b.text).unwrap().textgrid;
    let replan = plan_return(&p, &b.text, &again.textgrid, &again.manifest, edited_now.as_bytes()).unwrap();
    assert!(replan.changes.is_empty(), "{:?}", replan.changes);
    assert!(apply_return(&mut p, &b.text, &replan).unwrap().commit.is_none());

    // A crash after the commit and before the baseline moved on: the old
    // baseline again finds the adds already in wugs.
    let after_crash = plan_return(&p, &b.text, &h.textgrid, &h.manifest, EDITED).unwrap();
    assert!(after_crash.changes.iter().all(|c| !matches!(c, Change::Add { .. })), "{:?}", after_crash.changes);
    assert_eq!(after_crash.skipped.0.get("already in wugs"), Some(&3));

    // The window stays open in Praat. Against the rebased baseline the same
    // save brings nothing back twice, and a second session only its own
    // change.
    let (baseline, manifest) = plan.rebased(&applied).unwrap();
    let same = plan_return(&p, &b.text, &baseline, &manifest, EDITED).unwrap();
    assert!(same.changes.is_empty(), "nothing twice: {:?}", same.changes);
    let second = plan_return(&p, &b.text, &baseline, &manifest, EDITED_2).unwrap();
    let dort = p
        .proj
        .children_by_ref("text", &b.text, "segment")
        .unwrap()
        .into_iter()
        .find(|s| p.proj.field_str(s, "content").unwrap().as_deref() == Some("dört"))
        .unwrap();
    assert_eq!(second.changes, vec![Change::Retime { segment: dort, start_ms: None, end_ms: Some(1900.0) }]);
    assert_eq!(second.skipped.0.get("tier added in Praat"), Some(&1), "{:?}", second.skipped);

    // Edits made in wugs meanwhile are not Praat deletions.
    p.add_segment(&b.text, &b.utt, 99, Some(3600.0), Some(3900.0), "elsewhere").unwrap();
    let third = plan_return(&p, &b.text, &baseline, &manifest, EDITED_2).unwrap();
    assert_eq!(third.changes, second.changes);
    fs::remove_dir_all(root).ok();
}

fn tones_of(p: &Project, b: &Built) -> String {
    p.proj.field_str(&b.high, "tier").unwrap().unwrap()
}

#[test]
fn unchanged_grid_plans_nothing_and_segments_deleted_meanwhile_are_skipped() {
    let (root, mut p) = temp_project();
    let b = text(&mut p);
    let h = hand_out(&p, &b.text).unwrap();
    let plan = plan_return(&p, &b.text, &h.textgrid, &h.manifest, h.textgrid.as_bytes()).unwrap();
    assert!(plan.changes.is_empty(), "{:?}", plan.changes);

    // Deleted in wugs while the grid was in Praat, then edited in Praat.
    p.delete_segment(&b.text, &b.bir).unwrap();
    let edited = h.textgrid.replace("\"bir\"", "\"biri\"");
    let plan = plan_return(&p, &b.text, &h.textgrid, &h.manifest, edited.as_bytes()).unwrap();
    assert!(plan.changes.is_empty(), "{:?}", plan.changes);
    assert_eq!(plan.skipped.0.get("segment deleted in wugs since it went to Praat"), Some(&1));
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_manifest_for_another_text_or_a_mismatched_grid_is_refused() {
    let (root, mut p) = temp_project();
    let b = text(&mut p);
    let other = p.create_text("Other").unwrap();
    let h = hand_out(&p, &b.text).unwrap();
    assert!(plan_return(&p, &other, &h.textgrid, &h.manifest, h.textgrid.as_bytes()).is_err());
    let other_grid = hand_out(&p, &other).unwrap().textgrid;
    assert!(plan_return(&p, &b.text, &other_grid, &h.manifest, h.textgrid.as_bytes()).is_err());
    assert!(plan_return(&p, &b.text, &h.textgrid, "{not json", h.textgrid.as_bytes()).is_err());
    fs::remove_dir_all(root).ok();
}
