//! Import -> export -> diff harness for Praat TextGrids.

use std::fs;
use std::path::PathBuf;

use wugs_core::{Project, SCOPE_CONFIG};
use wugs_legacy::{decode, export_textgrid, import_textgrid, import_toolbox};
use wugs_model::Value;

const LONG: &str = include_str!("fixtures/sample.TextGrid");
const SHORT: &str = include_str!("fixtures/sample-short.TextGrid");

fn temp_project() -> (PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("wugs-legacy-tg-{}", wugs_model::ulid()));
    let p = Project::open(&root, "prs_Tester").unwrap();
    (root, p)
}

fn counts(p: &Project) -> (i64, i64, i64) {
    (
        p.proj.count("entities").unwrap(),
        p.proj.count("fields").unwrap(),
        p.proj.count("ops_applied").unwrap(),
    )
}

/// A TextGrid read straight from text, independent of the importer: the
/// values in file order (numbers, strings, flags), then the grammar.
#[derive(Debug, PartialEq, Clone)]
enum V {
    N(f64),
    S(String),
    F(String),
}

fn values(src: &str) -> Vec<V> {
    let b: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == '"' {
            let mut s = String::new();
            i += 1;
            loop {
                if b[i] == '"' && b.get(i + 1) == Some(&'"') {
                    s.push('"');
                    i += 2;
                } else if b[i] == '"' {
                    break;
                } else {
                    s.push(b[i]);
                    i += 1;
                }
            }
            out.push(V::S(s));
        } else if c == '<' {
            let j = b[i..].iter().position(|&x| x == '>').unwrap() + i;
            out.push(V::F(b[i + 1..j].iter().collect()));
            i = j;
        } else if c == '[' {
            i += b[i..].iter().position(|&x| x == ']').unwrap();
        } else if c.is_ascii_digit() || c == '-' {
            let j = b[i..].iter().position(|x| !(x.is_ascii_digit() || *x == '.' || *x == '-')).map_or(b.len(), |p| p + i);
            out.push(V::N(b[i..j].iter().collect::<String>().parse().unwrap()));
            i = j - 1;
        } else if c.is_alphabetic() {
            while i + 1 < b.len() && b[i + 1].is_alphanumeric() {
                i += 1;
            }
        }
        i += 1;
    }
    out
}

#[derive(Debug, PartialEq)]
struct Tier {
    class: String,
    name: String,
    xmin: f64,
    xmax: f64,
    /// Every interval or point as written, empty ones included.
    all: Vec<(f64, f64, String)>,
}

#[derive(Debug, PartialEq)]
struct Grid {
    xmin: f64,
    xmax: f64,
    tiers: Vec<Tier>,
}

fn read(src: &str) -> Grid {
    let v = values(src);
    assert_eq!(v[0], V::S("ooTextFile".into()));
    assert_eq!(v[1], V::S("TextGrid".into()));
    let mut k = 2;
    let num = |k: &mut usize| {
        *k += 1;
        match &v[*k - 1] {
            V::N(x) => *x,
            o => panic!("number expected at {}, got {o:?}", *k - 1),
        }
    };
    let st = |k: &mut usize| {
        *k += 1;
        match &v[*k - 1] {
            V::S(x) => x.clone(),
            o => panic!("string expected at {}, got {o:?}", *k - 1),
        }
    };
    let xmin = num(&mut k);
    let xmax = num(&mut k);
    let mut tiers = Vec::new();
    if v[k] == V::F("exists".into()) {
        k += 1;
        for _ in 0..num(&mut k) as usize {
            let class = st(&mut k);
            let name = st(&mut k);
            let (a, b) = (num(&mut k), num(&mut k));
            let mut all = Vec::new();
            for _ in 0..num(&mut k) as usize {
                if class == "IntervalTier" {
                    let (s, e) = (num(&mut k), num(&mut k));
                    all.push((s, e, st(&mut k)));
                } else {
                    let t = num(&mut k);
                    all.push((t, t, st(&mut k)));
                }
            }
            tiers.push(Tier { class, name, xmin: a, xmax: b, all });
        }
    } else {
        k += 1;
    }
    assert!(v[k..].is_empty(), "trailing values {:?}", &v[k..]);
    Grid { xmin, xmax, tiers }
}

/// What wugs carries: the grid, tiers, labelled intervals and every point
/// (an empty mark is still a point; only empty intervals are gap filler).
fn canon(src: &str) -> Grid {
    let mut g = read(src);
    for t in &mut g.tiers {
        if t.class == "IntervalTier" {
            t.all.retain(|i| !i.2.is_empty());
        }
    }
    g
}

/// Praat refuses interval tiers that do not tile [xmin, xmax] exactly.
fn assert_praat_valid(src: &str) {
    let g = read(src);
    for t in &g.tiers {
        assert!(t.xmin >= g.xmin && t.xmax <= g.xmax, "tier {} inside the grid", t.name);
        if t.class == "IntervalTier" {
            assert!(!t.all.is_empty(), "tier {} has at least one interval", t.name);
            assert_eq!(t.all[0].0, t.xmin, "tier {} starts at xmin", t.name);
            assert_eq!(t.all.last().unwrap().1, t.xmax, "tier {} ends at xmax", t.name);
            for w in t.all.windows(2) {
                assert_eq!(w[0].1, w[1].0, "tier {} contiguous at {:?} | {:?}", t.name, w[0], w[1]);
            }
            for i in &t.all {
                assert!(i.0 < i.1, "tier {}: interval {i:?} has positive length", t.name);
            }
        } else {
            for w in t.all.windows(2) {
                assert!(w[0].0 < w[1].0, "tier {}: points strictly increasing", t.name);
            }
        }
    }
}

#[test]
fn fixtures_read_as_intended() {
    let g = read(LONG);
    assert_eq!(g, read(SHORT), "long and short forms hold the same grid");
    assert_eq!(g.tiers.len(), 3);
    assert_eq!(g.tiers[0].all[2].2, "geldi \"dedi\"", "doubled quotes decode");
    assert_eq!(g.tiers[1].all.len(), 1, "notes tier is one empty interval");
    assert_eq!(g.tiers[2].all[1], (1.6, 1.6, String::new()), "tones tier has an empty-mark point");
    assert_praat_valid(LONG);
}

#[test]
fn round_trip_is_semantically_identical_and_praat_valid() {
    let (root, mut p) = temp_project();
    let r = import_textgrid(&mut p, LONG, "Elicitation 1").unwrap();
    assert_eq!(
        (r.tiers_created, r.segments_created, r.empty_intervals_skipped, r.non_tiling_intervals),
        (3, 6, 3, 0),
        "three tiers, three labelled intervals and three points (one empty), three empty intervals"
    );
    let out = export_textgrid(&p, &r.text).unwrap();
    assert_eq!(out.not_exported.total(), 0, "nothing withheld");
    assert_eq!(canon(&out.textgrid), canon(LONG), "export matches the original");
    assert_eq!(read(&out.textgrid), read(LONG), "gap filling reproduces the original empty intervals too");
    assert_praat_valid(&out.textgrid);
    fs::remove_dir_all(root).ok();
}

#[test]
fn times_are_ms_and_labels_unicode() {
    let (root, mut p) = temp_project();
    let r = import_textgrid(&mut p, LONG, "T").unwrap();
    let mut segs: Vec<(f64, f64, String)> = p
        .proj
        .children_by_ref("text", &r.text, "segment")
        .unwrap()
        .iter()
        .map(|s| {
            (
                p.proj.field_num(s, "start").unwrap().unwrap(),
                p.proj.field_num(s, "end").unwrap().unwrap(),
                p.proj.field_str(s, "content").unwrap().unwrap(),
            )
        })
        .collect();
    segs.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(
        segs,
        vec![
            (421.0, 1250.0, "köpek".into()),
            (800.0, 800.0, "H*".into()),
            (1250.0, 2000.0, "geldi \"dedi\"".into()),
            (1600.0, 1600.0, String::new()),
            (2350.0, 2350.0, "L%".into()),
            (2700.0, 3500.0, "şey 🐕".into()),
        ],
        "seconds rounded to ms; points have start == end"
    );
    assert_eq!(r.findings, 0, "imported segments pass the validators");
    fs::remove_dir_all(root).ok();
}

#[test]
fn reimport_is_idempotent_across_forms_and_own_export() {
    let (root, mut p) = temp_project();
    let first = import_textgrid(&mut p, LONG, "T").unwrap();
    let before = counts(&p);
    for (what, src) in [("same file", LONG.to_string()), ("short form", SHORT.to_string()), ("own export", export_textgrid(&p, &first.text).unwrap().textgrid)] {
        let r = import_textgrid(&mut p, &src, "T").unwrap();
        assert_eq!(r.text, first.text, "{what} resolves to the same text");
        assert_eq!((r.tiers_created, r.segments_created, r.text_created), (0, 0, false), "{what} creates nothing");
    }
    assert_eq!(counts(&p), before, "counts unchanged");
    fs::remove_dir_all(root).ok();
}

#[test]
fn export_from_replayed_log_matches() {
    let (root, mut p) = temp_project();
    let r = import_textgrid(&mut p, LONG, "T").unwrap();
    drop(p);
    fs::remove_dir_all(root.join("cache")).unwrap();
    let p2 = Project::open(&root, "prs_Tester").unwrap();
    assert_eq!(read(&export_textgrid(&p2, &r.text).unwrap().textgrid), read(LONG), "replayed projection exports the same grid");
    fs::remove_dir_all(root).ok();
}

#[test]
fn edits_and_born_digital_texts_export_as_valid_textgrids() {
    let (root, mut p) = temp_project();
    let r = import_textgrid(&mut p, LONG, "T").unwrap();
    let kopek = p
        .proj
        .children_by_ref("text", &r.text, "segment")
        .unwrap()
        .into_iter()
        .find(|s| p.proj.field_str(s, "content").unwrap().as_deref() == Some("köpek"))
        .unwrap();
    let mut t = p.intent("retime");
    t.set("segment", &kopek, &r.text, "end", Value::Num(1100.0)).unwrap();
    t.commit().unwrap();
    let out = export_textgrid(&p, &r.text).unwrap().textgrid;
    assert_praat_valid(&out);
    let words = &read(&out).tiers[0].all;
    assert!(words.contains(&(0.4213, 1.1, "köpek".into())), "edited end wins, untouched start keeps its digits: {words:?}");
    assert!(words.contains(&(1.1, 1.25, String::new())), "new gap filled: {words:?}");

    let mut t = p.intent("create-tier");
    let tier = t.create("tier", "tir", SCOPE_CONFIG, vec![("name", Value::str("utt"))]);
    t.commit().unwrap();
    let txt = p.create_text("Born digital").unwrap();
    p.add_segment(&txt, &tier, 0, Some(500.0), Some(1200.0), "bir").unwrap();
    p.add_segment(&txt, &tier, 1, Some(2000.0), Some(2600.0), "iki").unwrap();
    p.add_segment(&txt, &tier, 2, Some(2400.0), Some(3000.0), "overlap").unwrap();
    p.add_segment(&txt, &tier, 3, None, None, "untimed").unwrap();
    let born = export_textgrid(&p, &txt).unwrap();
    assert_praat_valid(&born.textgrid);
    let g = read(&born.textgrid);
    assert_eq!(
        g.tiers[0].all,
        vec![(0.0, 0.5, "".into()), (0.5, 1.2, "bir".into()), (1.2, 2.0, "".into()), (2.0, 2.6, "iki".into()), (2.6, 3.0, "".into())],
        "gaps filled from 0 to the text's last end (the withheld overlap still spans the text)"
    );
    assert_eq!(born.not_exported.total(), 2, "overlap and untimed segment counted: {:?}", born.not_exported);
    fs::remove_dir_all(root).ok();
}

#[test]
fn invalid_input_is_rejected_without_writes() {
    let (root, mut p) = temp_project();
    let before = counts(&p);
    let truncated = &LONG[..LONG.len() / 2];
    let bad_class = LONG.replace("\"TextTier\"", "\"PitchTier\"");
    let backwards = LONG.replace("xmax = 1.25", "xmax = 0.1");
    let unterminated = LONG.replace("text = \"köpek\"", "text = \"köpek");
    let trailing = format!("{LONG}\n42\n");
    for (what, bad) in [
        ("empty", ""),
        ("binary", "File type = \"ooBinaryFile\"\nObject class = \"TextGrid\"\n"),
        ("wrong object", "File type = \"ooTextFile\"\nObject class = \"Pitch 1\"\n0\n1\n"),
        ("truncated", truncated),
        ("unknown tier class", &bad_class),
        ("xmax before xmin", &backwards),
        ("unterminated string", &unterminated),
        ("trailing data", &trailing),
    ] {
        assert!(import_textgrid(&mut p, bad, "T").is_err(), "must reject {what}");
    }
    assert_eq!(counts(&p), before, "rejected input leaves the project untouched");
    fs::remove_dir_all(root).ok();
}

#[test]
fn utf16_files_decode() {
    let (root, mut p) = temp_project();
    let mut le = vec![0xFF, 0xFE];
    le.extend(LONG.encode_utf16().flat_map(u16::to_le_bytes));
    let mut be = vec![0xFE, 0xFF];
    be.extend(SHORT.encode_utf16().flat_map(u16::to_be_bytes));
    let a = import_textgrid(&mut p, &decode(&le).unwrap(), "T").unwrap();
    let b = import_textgrid(&mut p, &decode(&be).unwrap(), "T").unwrap();
    assert_eq!(a.segments_created, 6);
    assert_eq!(b.text, a.text, "UTF-16 BE short form is the same text");
    assert!(decode(&[0xFF, 0xFE, 0x41]).is_err(), "odd UTF-16 rejected");
    assert_eq!(decode(b"caf\xe9").unwrap(), "café", "bytes that are not UTF-8 are Latin-1, which Praat can be set to write");
    fs::remove_dir_all(root).ok();
}

#[test]
fn every_import_txn_stays_in_one_scope() {
    let (root, mut p) = temp_project();
    import_textgrid(&mut p, LONG, "T").unwrap();
    import_toolbox(&mut p, include_str!("fixtures/sample.db")).unwrap();
    let mut scopes: std::collections::HashMap<String, std::collections::HashSet<String>> = Default::default();
    for op in p.log.read_all().unwrap().ops {
        scopes.entry(op.txn).or_default().insert(op.scope);
    }
    for (txn, s) in &scopes {
        assert_eq!(s.len(), 1, "txn {txn} spans scopes {s:?}");
    }
    fs::remove_dir_all(root).ok();
}

fn grid(tiers: &str, n: usize) -> String {
    format!("File type = \"ooTextFile\"\nObject class = \"TextGrid\"\n0\n3\n<exists>\n{n}\n{tiers}")
}

#[test]
fn empty_mark_points_survive_the_round_trip() {
    let (root, mut p) = temp_project();
    let src = grid("\"TextTier\"\n\"beats\"\n0\n3\n3\n0.5\n\"\"\n1\n\"\"\n2\n\"x\"\n", 1);
    let r = import_textgrid(&mut p, &src, "T").unwrap();
    assert_eq!((r.segments_created, r.empty_intervals_skipped), (3, 0), "empty marks are points, not gap filler");
    let out = export_textgrid(&p, &r.text).unwrap();
    assert_eq!(read(&out.textgrid), read(&src), "every point exported, empty marks included");
    fs::remove_dir_all(root).ok();
}

#[test]
fn sub_ms_overlap_in_the_source_keeps_both_labels() {
    let (root, mut p) = temp_project();
    let src = grid("\"IntervalTier\"\n\"w\"\n0\n3\n2\n0\n1.2500000001\n\"a\"\n1.25\n3\n\"b\"\n", 1);
    let r = import_textgrid(&mut p, &src, "T").unwrap();
    assert_eq!(r.non_tiling_intervals, 1, "the overlap is reported at import");
    let out = export_textgrid(&p, &r.text).unwrap();
    assert_eq!(out.not_exported.total(), 0, "nothing withheld: {:?}", out.not_exported);
    assert_praat_valid(&out.textgrid);
    assert_eq!(
        read(&out.textgrid).tiers[0].all,
        vec![(0.0, 1.2500000001, "a".into()), (1.2500000001, 3.0, "b".into())],
        "b starts where a ends"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn boundary_snapped_to_the_ms_keeps_both_labels() {
    let (root, mut p) = temp_project();
    let src = grid("\"IntervalTier\"\n\"w\"\n0\n3\n3\n0\n0.4213\n\"a\"\n0.4213\n0.5\n\"\"\n0.5\n3\n\"b\"\n", 1);
    let r = import_textgrid(&mut p, &src, "T").unwrap();
    assert_eq!(r.non_tiling_intervals, 0);
    let b = p
        .proj
        .children_by_ref("text", &r.text, "segment")
        .unwrap()
        .into_iter()
        .find(|s| p.proj.field_str(s, "content").unwrap().as_deref() == Some("b"))
        .unwrap();
    let mut t = p.intent("retime");
    t.set("segment", &b, &r.text, "start", Value::Num(421.0)).unwrap();
    t.commit().unwrap();
    let out = export_textgrid(&p, &r.text).unwrap();
    assert_eq!(out.not_exported.total(), 0, "nothing withheld: {:?}", out.not_exported);
    assert_praat_valid(&out.textgrid);
    assert_eq!(read(&out.textgrid).tiers[0].all, vec![(0.0, 0.4213, "a".into()), (0.4213, 3.0, "b".into())]);
    fs::remove_dir_all(root).ok();
}

#[test]
fn blank_grids_of_the_same_shape_are_separate_texts() {
    let (root, mut p) = temp_project();
    let blank = grid("\"IntervalTier\"\n\"w\"\n0\n3\n1\n0\n3\n\"\"\n", 1);
    let a = import_textgrid(&mut p, &blank, "A").unwrap();
    let b = import_textgrid(&mut p, &blank, "B").unwrap();
    assert!(a.text_created && b.text_created, "each blank grid gets its own text");
    assert_ne!(a.text, b.text);
    let first = import_textgrid(&mut p, LONG, "T").unwrap();
    let again = import_textgrid(&mut p, LONG, "T").unwrap();
    assert!(first.text_created && !again.text_created, "a labelled grid still resolves to its text, and says so");
    fs::remove_dir_all(root).ok();
}
