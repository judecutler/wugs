//! Import -> export -> diff harness (PRD-TONGUES.md §5).

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use wugs_core::{Project, SCOPE_CONFIG};
use wugs_elan::xml::{self, XmlNode};
use wugs_elan::{export_eaf, import_eaf};
use wugs_model::Value;

const SAMPLE: &str = include_str!("fixtures/sample.eaf");

fn temp_project() -> (PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("wugs-elan-test-{}", wugs_model::ulid()));
    let p = Project::open(&root, "prs_Tester").unwrap();
    (root, p)
}

#[derive(Debug, PartialEq)]
struct CanonTier {
    name: String,
    kind: String,
    parent: Option<String>,
    anns: Vec<CanonAnn>,
}

#[derive(Debug, PartialEq)]
struct CanonAnn {
    content: String,
    start: Option<i64>,
    end: Option<i64>,
    parent_content: Option<String>,
}

/// Reads an EAF straight from the XML, independent of the importer, so a
/// shared misreading cannot make the round trip pass.
fn canon(eaf: &str) -> Vec<CanonTier> {
    let root = xml::parse(eaf).unwrap();
    let slots: HashMap<&str, Option<i64>> = root
        .child("TIME_ORDER")
        .unwrap()
        .children_named("TIME_SLOT")
        .map(|s| {
            (
                s.attr("TIME_SLOT_ID").unwrap(),
                s.attr("TIME_VALUE").map(|v| v.parse().unwrap()),
            )
        })
        .collect();
    let types: HashMap<&str, &XmlNode> = root
        .children_named("LINGUISTIC_TYPE")
        .map(|t| (t.attr("LINGUISTIC_TYPE_ID").unwrap(), t))
        .collect();
    let mut content_of: HashMap<&str, &str> = HashMap::new();
    for a in root.children_named("TIER").flat_map(|t| t.children_named("ANNOTATION")) {
        let inner = &a.children[0];
        content_of.insert(
            inner.attr("ANNOTATION_ID").unwrap(),
            &inner.child("ANNOTATION_VALUE").unwrap().text,
        );
    }

    let mut tiers: Vec<CanonTier> = root
        .children_named("TIER")
        .map(|t| {
            let lt = types[t.attr("LINGUISTIC_TYPE_REF").unwrap()];
            let kind = match lt.attr("CONSTRAINTS") {
                Some("Symbolic_Subdivision") => "symbolic-subdivision",
                Some("Symbolic_Association") => "symbolic-association",
                _ => "time-aligned",
            };
            let anns = t
                .children_named("ANNOTATION")
                .map(|a| {
                    let inner = &a.children[0];
                    let content = inner.child("ANNOTATION_VALUE").unwrap().text.clone();
                    match inner.attr("ANNOTATION_REF") {
                        Some(r) => CanonAnn {
                            content,
                            start: None,
                            end: None,
                            parent_content: Some(content_of[r].to_string()),
                        },
                        None => CanonAnn {
                            content,
                            start: slots[inner.attr("TIME_SLOT_REF1").unwrap()],
                            end: slots[inner.attr("TIME_SLOT_REF2").unwrap()],
                            parent_content: None,
                        },
                    }
                })
                .collect();
            CanonTier {
                name: t.attr("TIER_ID").unwrap().to_string(),
                kind: kind.to_string(),
                parent: t.attr("PARENT_REF").map(str::to_string),
                anns,
            }
        })
        .collect();
    tiers.sort_by(|a, b| a.name.cmp(&b.name));
    tiers
}

fn entity_counts(p: &Project) -> (i64, i64, i64) {
    (
        p.proj.count("entities").unwrap(),
        p.proj.count("fields").unwrap(),
        p.proj.count("ops_applied").unwrap(),
    )
}

/// Scope-filtered sync delivers a txn only partly when its ops span scopes.
fn assert_one_scope_per_txn(p: &Project) {
    let mut scopes: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    for op in p.log.read_all().unwrap().ops {
        scopes.entry(op.txn).or_default().insert(op.scope);
    }
    assert!(scopes.len() > 3, "several txns written");
    for (txn, s) in &scopes {
        assert_eq!(s.len(), 1, "txn {txn} spans scopes {s:?}, which scope-filtered sync would split");
    }
}

#[test]
fn every_import_txn_stays_in_one_scope() {
    let (root, mut p) = temp_project();
    let r = import_eaf(&mut p, SAMPLE, "T").unwrap();
    let exported = export_eaf(&p, &r.text).unwrap();
    import_eaf(&mut p, &exported, "T").unwrap();
    assert_one_scope_per_txn(&p);

    let text_ops: Vec<_> = p.log.read_all().unwrap().ops.into_iter().filter(|o| o.target == r.text).collect();
    assert_eq!(text_ops.len(), 1, "text is born in one create op, no follow-up sets");
    assert!(text_ops[0].value.get_str("origin").is_some(), "origin rides on the text's create op");
    fs::remove_dir_all(root).ok();
}

#[test]
fn fixture_reads_as_intended() {
    let tiers = canon(SAMPLE);
    assert_eq!(tiers.len(), 3, "fixture has three tiers");
    assert_eq!(tiers.iter().map(|t| t.anns.len()).sum::<usize>(), 10, "fixture has ten annotations");
    let utt = tiers.iter().find(|t| t.name == "utterance").unwrap();
    assert_eq!(utt.anns[0].content, "köpekler geldi", "Turkish content is read intact");
    assert_eq!(utt.anns[1].content, "Ali & Ayşe eve gitti", "&amp; is decoded");
    assert_eq!(utt.anns[2].start, None, "ts4 has no TIME_VALUE");
    assert_eq!(utt.anns[2].end, Some(5000), "ts5 keeps its value");
}

#[test]
fn round_trip_is_semantically_identical() {
    let (root, mut p) = temp_project();
    let report = import_eaf(&mut p, SAMPLE, "Elicitation 01").unwrap();
    assert_eq!((report.tiers_created, report.segments_created), (3, 10), "everything imported once");

    let exported = export_eaf(&p, &report.text).unwrap();
    assert_eq!(canon(&exported), canon(SAMPLE), "exported EAF matches the original per tier");

    let txt = &report.text;
    assert_eq!(p.proj.field_str(txt, "title").unwrap().unwrap(), "Elicitation 01", "title kept");
    let utt_seg = p
        .proj
        .children_by_ref("text", txt, "segment")
        .unwrap()
        .into_iter()
        .find(|s| p.proj.field_str(s, "origin").unwrap().as_deref() == Some("a3"))
        .unwrap();
    assert_eq!(p.proj.field_num(&utt_seg, "start").unwrap(), None, "unaligned slot stays absent, no interpolation");
    assert_eq!(p.proj.field_num(&utt_seg, "end").unwrap(), Some(5000.0), "aligned side kept");
    fs::remove_dir_all(root).ok();
}

#[test]
fn reimport_is_idempotent() {
    let (root, mut p) = temp_project();
    let first = import_eaf(&mut p, SAMPLE, "T").unwrap();
    let exported = export_eaf(&p, &first.text).unwrap();
    let before = entity_counts(&p);

    let again = import_eaf(&mut p, &exported, "T").unwrap();
    assert_eq!(again.text, first.text, "export of a project text resolves to that text");
    assert_eq!((again.tiers_created, again.segments_created), (0, 0), "nothing created from our own export");
    assert_eq!(entity_counts(&p), before, "entity, field and op counts unchanged after re-import of export");

    let twice = import_eaf(&mut p, SAMPLE, "T").unwrap();
    assert_eq!(twice.text, first.text, "same original file resolves to same text");
    assert_eq!(twice.segments_created, 0, "no duplicate segments from the original");
    assert_eq!(entity_counts(&p), before, "counts unchanged after re-import of the original");
    fs::remove_dir_all(root).ok();
}

#[test]
fn import_survives_replay_from_the_log() {
    let (root, mut p) = temp_project();
    let r = import_eaf(&mut p, SAMPLE, "T").unwrap();
    drop(p);
    fs::remove_dir_all(root.join("cache")).unwrap();
    let p2 = Project::open(&root, "prs_Tester").unwrap();
    assert_eq!(canon(&export_eaf(&p2, &r.text).unwrap()), canon(SAMPLE), "export from replayed projection matches");
    fs::remove_dir_all(root).ok();
}

#[test]
fn born_digital_text_exports_and_reimports() {
    let (root_a, mut a) = temp_project();
    let mut t = a.intent("create-tier");
    let utt = t.create(
        "tier",
        "tir",
        SCOPE_CONFIG,
        vec![("name", Value::str("utterance")), ("kind", Value::str("time-aligned"))],
    );
    t.commit().unwrap();
    let txt = a.create_text("Born digital").unwrap();
    a.add_segment(&txt, &utt, 0, Some(0.0), Some(1500.0), "köpekler geldi").unwrap();
    a.add_segment(&txt, &utt, 1, Some(1500.0), Some(2500.0), "Ali & Ayşe <geldi>").unwrap();
    a.add_segment(&txt, &utt, 2, None, None, "untimed").unwrap();

    let exported = export_eaf(&a, &txt).unwrap();
    let (root_b, mut b) = temp_project();
    let r = import_eaf(&mut b, &exported, "fallback").unwrap();
    assert_eq!((r.tiers_created, r.segments_created), (1, 3), "fresh project gets the tier and segments");
    assert_ne!(r.text, txt, "fresh project mints its own text id");

    let rows = |p: &Project, text: &str| {
        let mut v: Vec<(String, String, Option<f64>, Option<f64>)> = p
            .proj
            .children_by_ref("text", text, "segment")
            .unwrap()
            .iter()
            .map(|s| {
                (
                    p.proj.field_str(s, "pos").unwrap().unwrap(),
                    p.proj.field_str(s, "content").unwrap().unwrap(),
                    p.proj.field_num(s, "start").unwrap(),
                    p.proj.field_num(s, "end").unwrap(),
                )
            })
            .collect();
        v.sort_by(|x, y| x.0.cmp(&y.0));
        v
    };
    assert_eq!(rows(&b, &r.text), rows(&a, &txt), "pos, content and times survive");
    assert_eq!(rows(&b, &r.text)[2].2, None, "untimed segment stays untimed");
    assert_eq!(b.proj.field_str(&r.text, "title").unwrap().unwrap(), "Born digital", "title travels in the header");
    assert_eq!(canon(&export_eaf(&b, &r.text).unwrap()), canon(&exported), "second export equals first");
    fs::remove_dir_all(root_a).ok();
    fs::remove_dir_all(root_b).ok();
}

#[test]
fn invalid_eaf_is_rejected_without_writes() {
    let (root, mut p) = temp_project();
    let before = entity_counts(&p);
    for bad in [
        "<a>",
        "<NOT_EAF/>",
        "<ANNOTATION_DOCUMENT><TIER TIER_ID=\"t\" PARENT_REF=\"missing\"/></ANNOTATION_DOCUMENT>",
        "<ANNOTATION_DOCUMENT><TIER TIER_ID=\"t\"><ANNOTATION><ALIGNABLE_ANNOTATION ANNOTATION_ID=\"a\" TIME_SLOT_REF1=\"x\" TIME_SLOT_REF2=\"y\"/></ANNOTATION></TIER></ANNOTATION_DOCUMENT>",
    ] {
        assert!(import_eaf(&mut p, bad, "T").is_err(), "must reject {bad}");
    }
    assert_eq!(entity_counts(&p), before, "rejected input leaves the project untouched");
    fs::remove_dir_all(root).ok();
}
