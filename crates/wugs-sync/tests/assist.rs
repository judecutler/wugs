//! Machine transcription (roadmap item 27): a suggestion fills only an
//! empty segment, and the segment stays marked unverified exactly while the
//! machine's write is what it shows, on every device.

use std::fs;
use std::path::PathBuf;

use wugs_core::assist::{record_field, MachineTranscript, MACHINE_INTENT};
use wugs_core::{Engines, Project};
use wugs_model::Value;
use wugs_sync::{sync_pair, SyncPolicy};

struct Rig {
    roots: Vec<PathBuf>,
}

impl Rig {
    fn new() -> Rig {
        Rig { roots: Vec::new() }
    }

    fn root(&mut self) -> PathBuf {
        let root = std::env::temp_dir().join(format!("wugs-assist-{}", wugs_model::ulid()));
        self.roots.push(root.clone());
        root
    }

    /// A text with one empty segment, then a second device that joined and synced.
    fn pair(&mut self) -> (Project, Project, String, String) {
        self.pair_with(false)
    }

    /// The same, with collaborative text merging on both devices (as the app runs).
    fn pair_merged(&mut self) -> (Project, Project, String, String) {
        self.pair_with(true)
    }

    fn pair_with(&mut self, merge: bool) -> (Project, Project, String, String) {
        let engines = || Engines { merger: merge.then(|| Box::new(wugs_text::LoroMerger) as Box<dyn wugs_model::TextMerger>), ..Default::default() };
        let mut a = Project::open_with(self.root(), "prs_Ana", engines()).unwrap();
        let text = a.create_text("Story").unwrap();
        let (tier, _) = a.add_tier("utterance", "time-aligned", Some("transcription"), None).unwrap();
        let (seg, _) = a.add_segment(&text, &tier, 0, Some(0.0), Some(1500.0), "").unwrap();
        let mut b = Project::open_joined_with(self.root(), "prs_Bo", &a.project_id, engines()).unwrap();
        sync(&mut a, &mut b);
        (a, b, text, seg)
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        for r in &self.roots {
            fs::remove_dir_all(r).ok();
        }
    }
}

fn sync(a: &mut Project, b: &mut Project) {
    let all = SyncPolicy::allow_all();
    sync_pair(a, b, &all, &all).unwrap();
}

fn heard(text: &str, confidence: f64) -> MachineTranscript {
    MachineTranscript {
        text: text.into(),
        model: "test-model".into(),
        confidence,
        phones: text.chars().filter(|c| !c.is_whitespace()).map(|c| (c.to_string(), confidence)).collect(),
        source: None,
    }
}

fn content(p: &Project, seg: &str) -> String {
    p.proj.field_str(seg, "content").unwrap().unwrap_or_default()
}

#[test]
fn a_suggestion_fills_an_empty_segment_and_marks_it() {
    let mut rig = Rig::new();
    let (mut a, _b, text, seg) = rig.pair();
    a.machine_transcribe(&text, &seg, &heard("kopek ler", 0.42)).unwrap();
    assert_eq!(content(&a, &seg), "kopek ler");
    let mark = a.machine_mark(&seg).unwrap().expect("unverified");
    assert_eq!(mark.model, "test-model");
    assert!((mark.confidence - 0.42).abs() < 1e-9);
    let (_, op, _) = a.proj.field_with_verb(&seg, "content").unwrap().unwrap();
    let (asr, _) = a.proj.field(&seg, &record_field(&op)).unwrap().unwrap();
    assert_eq!(asr.get("phones").unwrap().to_json().matches('[').count(), 1 + 8, "eight phones, each [symbol, confidence]");
}

#[test]
fn the_log_records_machine_provenance() {
    let mut rig = Rig::new();
    let (mut a, _b, text, seg) = rig.pair();
    a.machine_transcribe(&text, &seg, &heard("ab", 0.9)).unwrap();
    let ops = a.log.read_all().unwrap().ops;
    let content_op = ops.iter().find(|o| o.target == seg && o.field.as_deref() == Some("content")).unwrap();
    assert_eq!(content_op.intent.as_deref(), Some(MACHINE_INTENT));
    assert_eq!(content_op.heads, None, "a plain write: it settles no conflict");
}

#[test]
fn a_suggestion_never_overwrites_a_transcription() {
    let mut rig = Rig::new();
    let (mut a, _b, text, seg) = rig.pair();
    a.edit_content(&text, &seg, "köpekler").unwrap();
    assert!(a.machine_transcribe(&text, &seg, &heard("kopekler", 0.9)).is_err());
    assert_eq!(content(&a, &seg), "köpekler");
    assert_eq!(a.machine_mark(&seg).unwrap(), None);
    assert!(a.machine_transcribe(&text, &seg, &heard("   ", 0.9)).is_err(), "nothing heard is not written");
}

#[test]
fn a_person_editing_the_text_verifies_it() {
    let mut rig = Rig::new();
    let (mut a, _b, text, seg) = rig.pair();
    a.machine_transcribe(&text, &seg, &heard("kopek", 0.3)).unwrap();
    a.edit_content(&text, &seg, "köpek").unwrap();
    assert_eq!(a.machine_mark(&seg).unwrap(), None);
    assert_eq!(content(&a, &seg), "köpek");
}

#[test]
fn keeping_the_machine_text_unchanged_still_needs_a_person() {
    // Retyping the same text is a person's edit too: it verifies.
    let mut rig = Rig::new();
    let (mut a, _b, text, seg) = rig.pair();
    a.machine_transcribe(&text, &seg, &heard("kopek", 0.3)).unwrap();
    a.edit_content(&text, &seg, "kopek").unwrap();
    assert_eq!(a.machine_mark(&seg).unwrap(), None);
}

#[test]
fn a_verification_on_another_device_clears_the_mark_everywhere() {
    let mut rig = Rig::new();
    let (mut a, mut b, text, seg) = rig.pair();
    a.machine_transcribe(&text, &seg, &heard("kopek", 0.3)).unwrap();
    sync(&mut a, &mut b);
    assert!(b.machine_mark(&seg).unwrap().is_some(), "the mark travels with the suggestion");
    b.edit_content(&text, &seg, "köpek").unwrap();
    sync(&mut a, &mut b);
    for p in [&a, &b] {
        assert_eq!(p.machine_mark(&seg).unwrap(), None);
        assert_eq!(content(p, &seg), "köpek");
    }
}

#[test]
fn a_person_typing_while_the_machine_runs_elsewhere_wins_or_is_kept() {
    // Both write at once: B types, A's model fills the (to A) empty segment.
    // Whatever wins, the mark is shown only when the machine text shows.
    let mut rig = Rig::new();
    let (mut a, mut b, text, seg) = rig.pair();
    b.edit_content(&text, &seg, "köpek").unwrap();
    a.machine_transcribe(&text, &seg, &heard("kopek", 0.3)).unwrap();
    sync(&mut a, &mut b);
    for p in [&a, &b] {
        let shows_machine = content(p, &seg) == "kopek";
        assert_eq!(p.machine_mark(&seg).unwrap().is_some(), shows_machine);
    }
    assert_eq!(content(&a, &seg), content(&b, &seg));
}

#[test]
fn replay_derives_the_same_marks() {
    let mut rig = Rig::new();
    let (mut a, _b, text, seg) = rig.pair();
    a.machine_transcribe(&text, &seg, &heard("kopek", 0.3)).unwrap();
    let root = a.root.clone();
    drop(a);
    fs::remove_dir_all(root.join("cache")).unwrap();
    let a = Project::open(root, "prs_Ana").unwrap();
    assert!(a.machine_mark(&seg).unwrap().is_some());
}

fn conflicts(p: &Project, seg: &str) -> Vec<String> {
    let mut s = p.proj.db.prepare("SELECT field FROM conflicts WHERE target=?1 ORDER BY field").unwrap();
    s.bind_text(1, seg).unwrap();
    let mut out = Vec::new();
    while s.step().unwrap() {
        out.push(s.col_text(0));
    }
    out
}

#[test]
fn a_suggestion_into_a_cleared_segment_joins_its_text_history() {
    // Typed, cleared, suggested, then edited on both devices at once: the
    // edits merge into the machine text instead of adding it a second time.
    let mut rig = Rig::new();
    let (mut a, mut b, text, seg) = rig.pair_merged();
    a.edit_content(&text, &seg, "typo").unwrap();
    a.edit_content(&text, &seg, "").unwrap();
    a.machine_transcribe(&text, &seg, &heard("kopek", 0.4)).unwrap();
    assert!(a.machine_mark(&seg).unwrap().is_some());
    sync(&mut a, &mut b);
    a.edit_content(&text, &seg, "kopek.").unwrap();
    b.edit_content(&text, &seg, "köpek").unwrap();
    sync(&mut a, &mut b);
    assert_eq!(content(&a, &seg), content(&b, &seg));
    let merged = content(&a, &seg);
    assert_eq!(merged.matches('k').count(), 2, "one copy of the word, both edits in it: {merged:?}");
    assert_eq!(a.machine_mark(&seg).unwrap(), None);
}

#[test]
fn a_never_typed_segment_still_merges_cleanly_after_a_suggestion() {
    let mut rig = Rig::new();
    let (mut a, mut b, text, seg) = rig.pair_merged();
    a.machine_transcribe(&text, &seg, &heard("kopek", 0.4)).unwrap();
    sync(&mut a, &mut b);
    a.edit_content(&text, &seg, "kopek.").unwrap();
    b.edit_content(&text, &seg, "köpek").unwrap();
    sync(&mut a, &mut b);
    assert_eq!(content(&a, &seg), content(&b, &seg));
    assert_eq!(content(&a, &seg).matches('k').count(), 2, "{:?}", content(&a, &seg));
}

#[test]
fn two_devices_suggesting_at_once_give_one_conflict_and_a_true_mark() {
    let mut rig = Rig::new();
    let (mut a, mut b, text, seg) = rig.pair();
    a.machine_transcribe(&text, &seg, &heard("kopek", 0.4)).unwrap();
    b.machine_transcribe(&text, &seg, &heard("kobek", 0.7)).unwrap();
    sync(&mut a, &mut b);
    for p in [&a, &b] {
        assert_eq!(conflicts(p, &seg), ["content"], "the transcription conflicts; the records never do");
        let mark = p.machine_mark(&seg).unwrap().expect("machine text shows, so it is unverified");
        assert!((mark.confidence - 0.4).abs() < 1e-9, "both versions are the machine's: the less sure one");
    }
    // Settling it is a person's edit: verified.
    a.edit_content(&text, &seg, "köpek").unwrap();
    assert_eq!(a.machine_mark(&seg).unwrap(), None);
}

/// Both devices typed and cleared the segment and synced; then one fills it
/// by machine while the other types into it, before the next sync. The two
/// text-edits merge, so whichever is newer, the machine text shows unseen.
fn merged_machine_and_person(machine_first: bool) -> (Rig, Project, Project, String) {
    let mut rig = Rig::new();
    let (mut a, mut b, text, seg) = rig.pair_merged();
    a.edit_content(&text, &seg, "typo").unwrap();
    a.edit_content(&text, &seg, "").unwrap();
    sync(&mut a, &mut b);
    if machine_first {
        a.machine_transcribe(&text, &seg, &heard("kopek", 0.4)).unwrap();
        b.edit_content(&text, &seg, "ev").unwrap();
    } else {
        b.edit_content(&text, &seg, "ev").unwrap();
        a.machine_transcribe(&text, &seg, &heard("kopek", 0.4)).unwrap();
    }
    sync(&mut a, &mut b);
    (rig, a, b, seg)
}

#[test]
fn merged_machine_text_is_never_shown_as_checked() {
    for machine_first in [true, false] {
        let (_rig, a, b, seg) = merged_machine_and_person(machine_first);
        for p in [&a, &b] {
            assert!(content(p, &seg).contains("kopek"), "the machine text merged in: {:?}", content(p, &seg));
            assert!(p.machine_mark(&seg).unwrap().is_some(), "unseen machine text stays marked (machine first: {machine_first})");
        }
    }
}

#[test]
fn a_person_answering_merged_machine_text_verifies_it() {
    let (_rig, mut a, _b, seg) = merged_machine_and_person(false);
    let text = a.proj.field_str(&seg, "text").unwrap().unwrap();
    let shown = content(&a, &seg);
    a.edit_content(&text, &seg, &shown.replace("kopek", "köpek ")).unwrap();
    assert_eq!(a.machine_mark(&seg).unwrap(), None);
}

#[test]
fn an_empty_segment_without_a_conflict_still_takes_a_suggestion() {
    // Two devices clear the same text at once: two open versions, both
    // empty and merged, and no conflict to settle.
    let mut rig = Rig::new();
    let (mut a, mut b, text, seg) = rig.pair_merged();
    a.edit_content(&text, &seg, "typo").unwrap();
    sync(&mut a, &mut b);
    a.edit_content(&text, &seg, "").unwrap();
    b.edit_content(&text, &seg, "").unwrap();
    sync(&mut a, &mut b);
    assert!(conflicts(&a, &seg).is_empty());
    a.machine_transcribe(&text, &seg, &heard("kopek", 0.4)).unwrap();
}

#[test]
fn a_segment_empty_only_as_one_side_of_a_conflict_is_refused() {
    let mut rig = Rig::new();
    let (mut a, mut b, text, seg) = rig.pair();
    a.edit_content(&text, &seg, "köpek").unwrap();
    // B writes an empty value concurrently through the conflict path: set by hand.
    let mut t = b.intent("edit-content");
    t.set("segment", &seg, &text, "content", wugs_model::Value::str("")).unwrap();
    t.commit().unwrap();
    sync(&mut a, &mut b);
    for p in [&mut a, &mut b] {
        if content(p, &seg).is_empty() {
            let err = p.machine_transcribe(&text, &seg, &heard("kopek", 0.4)).unwrap_err();
            assert!(err.contains("conflict"), "{err}");
        }
    }
}

fn heard_from(text: &str, source: &str) -> MachineTranscript {
    MachineTranscript { source: Some(source.into()), ..heard(text, 0.4) }
}

#[test]
fn a_checked_suggestion_becomes_a_correction_pair() {
    let mut rig = Rig::new();
    let (mut a, mut b, text, seg) = rig.pair();
    let src = a.add_source("Recording", None, "audio", None).unwrap();
    a.machine_transcribe(&text, &seg, &heard_from("kopek ler", &src)).unwrap();
    let c = a.corrections().unwrap();
    assert!(c.pairs.is_empty(), "unchecked suggestions stay home");
    assert_eq!(c.unchecked, 1);

    // The person fixes it on the other device; both see the pair.
    sync(&mut a, &mut b);
    b.edit_content(&text, &seg, "köpekler").unwrap();
    sync(&mut a, &mut b);
    for p in [&a, &b] {
        let c = p.corrections().unwrap();
        assert_eq!(c.unchecked, 0);
        let pair = &c.pairs[..];
        assert_eq!(pair.len(), 1);
        assert_eq!((pair[0].machine.as_str(), pair[0].person.as_str()), ("kopek ler", "köpekler"));
        assert_eq!((pair[0].source.as_str(), pair[0].start_ms, pair[0].end_ms), (src.as_str(), 0.0, 1500.0));
        assert_eq!(pair[0].model, "test-model");
        assert!(!pair[0].retimed && !pair[0].consented);
    }
}

#[test]
fn consent_and_access_hold_corrections_back() {
    let mut rig = Rig::new();
    let (mut a, _b, text, seg) = rig.pair();
    let src = a.add_source("Recording", None, "audio", None).unwrap();
    a.machine_transcribe(&text, &seg, &heard_from("kopek", &src)).unwrap();
    a.edit_content(&text, &seg, "köpek").unwrap();
    let ana = a.add_person(Value::obj(vec![("full", Value::str("Ana"))]), Value::Arr(vec![]), "open").unwrap();

    a.record_consent(&ana, &src, Value::obj(vec![("AI-ok", Value::Bool(true))]), "2026-11-10").unwrap();
    assert!(a.corrections().unwrap().pairs[0].consented);

    // One refusal anywhere that covers it (here the whole project) wins.
    let project = a.project_id.clone();
    a.record_consent(&ana, &project, Value::obj(vec![("AI-ok", Value::Bool(false))]), "2026-11-10").unwrap();
    let c = a.corrections().unwrap();
    assert!(c.pairs.is_empty());
    assert_eq!(c.refused, 1);

    // A restricted recording stays home too.
    let mut rig2 = Rig::new();
    let (mut a, _b, text, seg) = rig2.pair();
    let src = a.add_source("Recording", None, "audio", Some("restricted")).unwrap();
    a.machine_transcribe(&text, &seg, &heard_from("kopek", &src)).unwrap();
    a.edit_content(&text, &seg, "köpek").unwrap();
    let c = a.corrections().unwrap();
    assert!(c.pairs.is_empty());
    assert_eq!(c.restricted, 1);
}

#[test]
fn a_retimed_or_unplaced_suggestion_is_flagged_or_left_out() {
    let mut rig = Rig::new();
    let (mut a, _b, text, seg) = rig.pair();
    let src = a.add_source("Recording", None, "audio", None).unwrap();
    a.machine_transcribe(&text, &seg, &heard_from("kopek", &src)).unwrap();
    a.set_segment_times(&text, &seg, Some(100.0), None).unwrap();
    a.edit_content(&text, &seg, "köpek").unwrap();
    let pair = a.corrections().unwrap().pairs.remove(0);
    assert!(pair.retimed, "heard on 0–1500, now 100–1500");
    assert_eq!(pair.start_ms, 100.0);

    // A suggestion that never said which recording it heard (written
    // before records kept it), in a text linked to none: nothing to cut.
    let mut c = Project::open_with(rig.root(), "prs_Cy", Engines::default()).unwrap();
    let t = c.create_text("Other").unwrap();
    let (tier, _) = c.add_tier("utterance", "time-aligned", Some("transcription"), None).unwrap();
    let (s, _) = c.add_segment(&t, &tier, 0, Some(0.0), Some(900.0), "").unwrap();
    c.machine_transcribe(&t, &s, &heard("ab", 0.5)).unwrap();
    c.edit_content(&t, &s, "abc").unwrap();
    let got = c.corrections().unwrap();
    assert!(got.pairs.is_empty());
    assert_eq!(got.unusable, 1);
}
