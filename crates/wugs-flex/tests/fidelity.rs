//! Import -> export -> diff harness for LIFT and FlexText (PRD-TONGUES.md §5).

use std::fs;
use std::path::PathBuf;

use wugs_core::Project;
use wugs_flex::{export_flextext, export_lift, import_flextext, import_lift};
use wugs_xml::{self as xml, XmlNode};

const LIFT: &str = include_str!("fixtures/sample.lift");
const FLEXTEXT: &str = include_str!("fixtures/sample.flextext");

fn temp_project() -> (PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("wugs-flex-test-{}", wugs_model::ulid()));
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

type Multi = Vec<(String, String)>;

fn attr(n: &XmlNode, k: &str) -> Option<String> {
    n.attr(k).map(str::to_string)
}

/// A GUID wugs derived on export for an entity that had none (version
/// nibble 8, variant a). Real FLEx guids are random (version 4).
fn is_derived_guid(g: &str) -> bool {
    g.len() == 36 && &g[14..15] == "8" && &g[19..20] == "a"
}

/// All attributes, sorted, minus derived guids, so "no guid in the source"
/// and "guid derived on export" compare equal.
fn attrs(n: &XmlNode) -> Vec<(String, String)> {
    let mut a: Vec<(String, String)> = n
        .attrs
        .iter()
        .filter(|(k, v)| !(k == "guid" && is_derived_guid(v)))
        .cloned()
        .collect();
    a.sort();
    a
}

fn forms(n: Option<&XmlNode>) -> Multi {
    n.map(|n| {
        n.children_named("form")
            .map(|f| (attr(f, "lang").unwrap(), f.child("text").unwrap().deep_text()))
            .collect()
    })
    .unwrap_or_default()
}

/// The LIFT content wugs carries, read straight from the XML.
#[derive(Debug, PartialEq)]
struct CanonEntry {
    id: String,
    attrs: Vec<(String, String)>,
    lexical_unit: Multi,
    citation: Multi,
    senses: Vec<(Vec<(String, String)>, Option<String>, Multi, Multi, Vec<CanonExample>)>,
    variants: Vec<(Vec<(String, String)>, Multi)>,
    pronunciations: Vec<(Vec<(String, String)>, Multi, Vec<Vec<(String, String)>>)>,
    /// Each `<relation>`'s attributes, the entry's then each sense's (sense id, attributes).
    relations: Vec<(String, Vec<(String, String)>)>,
}

/// An example: attributes, its forms, and each translation's type and forms.
#[derive(Debug, PartialEq)]
struct CanonExample(Vec<(String, String)>, Multi, Vec<(Option<String>, Multi)>);

fn canon_lift(s: &str) -> Vec<CanonEntry> {
    let root = xml::parse(s).unwrap();
    let mut v: Vec<CanonEntry> = root
        .children_named("entry")
        .map(|e| CanonEntry {
            id: attr(e, "id").unwrap(),
            attrs: attrs(e),
            lexical_unit: forms(e.child("lexical-unit")),
            citation: forms(e.child("citation")),
            senses: e
                .children_named("sense")
                .map(|s| {
                    (
                        attrs(s),
                        s.child("grammatical-info").and_then(|g| attr(g, "value")),
                        s.children_named("gloss")
                            .map(|g| (attr(g, "lang").unwrap(), g.child("text").unwrap().deep_text()))
                            .collect(),
                        forms(s.child("definition")),
                        s.children_named("example")
                            .map(|x| CanonExample(attrs(x), forms(Some(x)), x.children_named("translation").map(|t| (attr(t, "type"), forms(Some(t)))).collect()))
                            .collect(),
                    )
                })
                .collect(),
            variants: e.children_named("variant").map(|v| (attrs(v), forms(Some(v)))).collect(),
            pronunciations: e.children_named("pronunciation").map(|x| (attrs(x), forms(Some(x)), x.children_named("media").map(attrs).collect())).collect(),
            relations: {
                let mut r: Vec<(String, Vec<(String, String)>)> = e.children_named("relation").map(|x| (String::new(), attrs(x))).collect();
                for s in e.children_named("sense") {
                    r.extend(s.children_named("relation").map(|x| (attr(s, "id").unwrap_or_default(), attrs(x))));
                }
                r.sort();
                r
            },
        })
        .collect();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    v
}

type Items = Vec<(String, String, String)>;

fn items(n: &XmlNode) -> Items {
    n.children_named("item")
        .map(|i| (attr(i, "type").unwrap(), attr(i, "lang").unwrap_or_default(), i.deep_text()))
        .collect()
}

#[derive(Debug, PartialEq)]
struct CanonMorph(Vec<(String, String)>, Items);
#[derive(Debug, PartialEq)]
struct CanonWord(Vec<(String, String)>, Items, Vec<CanonMorph>);
#[derive(Debug, PartialEq)]
struct CanonPhrase(Vec<(String, String)>, Items, Vec<CanonWord>);
#[derive(Debug, PartialEq)]
struct CanonText {
    attrs: Vec<(String, String)>,
    items: Items,
    media: Option<String>,
    paragraphs: Vec<(Vec<(String, String)>, Vec<CanonPhrase>)>,
    languages: Vec<Vec<(String, String)>>,
}

fn canon_flextext(s: &str) -> Vec<CanonText> {
    let root = xml::parse(s).unwrap();
    root.children_named("interlinear-text")
        .map(|it| {
            let mut languages: Vec<Vec<(String, String)>> = it
                .child("languages")
                .map(|l| {
                    l.children_named("language")
                        .map(|x| {
                            let mut a = x.attrs.clone();
                            a.sort();
                            a
                        })
                        .collect()
                })
                .unwrap_or_default();
            languages.sort();
            CanonText {
                attrs: attrs(it),
                items: items(it),
                media: it.child("media-files").map(|m| xml::XmlNode { tail: String::new(), ..m.clone() }.to_xml()),
                languages,
                paragraphs: it
                    .child("paragraphs")
                    .unwrap()
                    .children_named("paragraph")
                    .map(|p| {
                        (
                            attrs(p),
                            p.child("phrases")
                                .unwrap()
                                .children_named("phrase")
                                .map(|ph| {
                                    CanonPhrase(
                                        attrs(ph),
                                        items(ph),
                                        ph.child("words")
                                            .map(|ws| {
                                                ws.children_named("word")
                                                    .map(|w| {
                                                        CanonWord(
                                                            attrs(w),
                                                            items(w),
                                                            w.child("morphemes")
                                                                .map(|ms| {
                                                                    ms.children_named("morph")
                                                                        .map(|m| CanonMorph(attrs(m), items(m)))
                                                                        .collect()
                                                                })
                                                                .unwrap_or_default(),
                                                        )
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default(),
                                    )
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            }
        })
        .collect()
}

#[test]
fn fixtures_read_as_intended() {
    let lift = canon_lift(LIFT);
    assert_eq!(lift.len(), 6, "six entries");
    let kopek = lift.iter().find(|e| e.id == "köpek_6f1a").unwrap();
    assert_eq!(kopek.lexical_unit.len(), 2, "two writing systems on the lemma");
    assert_eq!(kopek.senses[0].3[0].1, "a domesticated Canis animal", "span text read in order");
    let ft = canon_flextext(FLEXTEXT);
    assert_eq!(ft[0].paragraphs.len(), 2, "two paragraphs");
    assert_eq!(ft[0].paragraphs[0].1[0].2.len(), 3, "first phrase has two words and punctuation");
    assert!(ft[0].media.is_some(), "media-files present");
}

#[test]
fn lift_round_trip_is_semantically_identical() {
    let (root, mut p) = temp_project();
    let r = import_lift(&mut p, LIFT).unwrap();
    assert_eq!((r.entries_created, r.senses_created, r.variants_created, r.examples_created, r.pronunciations_created), (6, 7, 1, 2, 2), "everything imported once");
    assert_eq!(r.relations_created, 2, "the Compare pair FLEx lists on both ends is one relation, plus Calendar");
    assert_eq!(canon_lift(&export_lift(&p).unwrap()), canon_lift(LIFT), "exported LIFT matches the original");
    // Examples arrive as the sense's examples, editable like any.
    let exm = p.proj.entities_of_kind("example").unwrap();
    let sense = p.proj.field_str(&exm[0], "sense").unwrap().unwrap();
    let ex = p.examples(&sense).unwrap();
    assert_eq!((ex[0].sentence.as_str(), ex[0].translation.as_str(), ex[0].reference.as_deref()), ("Köpek havladı.", "The dog barked.", Some("Notebook 2, p. 14")));
    p.set_example_text(&ex[0].id, "translation", "The dog was barking.").unwrap();
    p.add_example(&sense, "Köpek koştu.", "The dog ran.", None, None).unwrap();
    let out = canon_lift(&export_lift(&p).unwrap());
    let kopek = &out.iter().find(|e| e.id == "köpek_6f1a").unwrap().senses[0].4;
    assert_eq!(kopek[0].2, vec![(Some("Free translation".to_string()), vec![("en".to_string(), "The dog was barking.".to_string()), ("de".to_string(), "Der Hund bellte.".to_string())])], "the edit replaces the first translation, the others stay");
    assert_eq!(kopek[2].1, vec![("tr".to_string(), "Köpek koştu.".to_string())], "a new example is written in the entry's language");
    assert_eq!(kopek[2].2, vec![(None, vec![("en".to_string(), "The dog ran.".to_string())])]);
    // Pronunciations arrive as the entry's, their media file names kept for the way back.
    let lex = p.proj.field_str(&sense, "lexeme").unwrap().unwrap();
    let said = p.pronunciations(&lex).unwrap();
    assert_eq!(said.iter().map(|x| x.form.as_str()).collect::<Vec<_>>(), ["cœˈpec", ""]);
    p.set_pronunciation_form(&said[1].id, "cœˈpek").unwrap();
    let gel_id = p.proj.entities_of_kind("lexeme").unwrap().into_iter().find(|l| p.proj.field_str(l, "lemma").unwrap().as_deref() == Some("gel")).unwrap();
    p.add_pronunciation(&gel_id, "ɟel", None).unwrap();
    let out = canon_lift(&export_lift(&p).unwrap());
    let kopek = &out.iter().find(|e| e.id == "köpek_6f1a").unwrap().pronunciations;
    assert_eq!(kopek[1].1, vec![("tr-fonipa".to_string(), "cœˈpek".to_string())], "a form typed into a media-only pronunciation");
    assert_eq!(kopek[1].2, vec![vec![("href".to_string(), "köpek-2.wav".to_string())]], "its media kept");
    let gel = &out.iter().find(|e| e.id == "gel_8c3d").unwrap().pronunciations;
    assert_eq!(gel[0].1, vec![("tr-fonipa".to_string(), "ɟel".to_string())]);
    // Relations read from both ends, and one added in wugs goes out under its entry.
    let yuz1 = p.proj.entities_of_kind("lexeme").unwrap().into_iter().find(|l| p.proj.field_str(l, "x.lift.id").unwrap().as_deref() == Some("yüz_1")).unwrap();
    let yuz2 = p.relations(&yuz1).unwrap()[0].other.clone();
    assert_eq!(p.relations(&yuz2).unwrap()[0].relation, "compare");
    p.add_relation(&yuz1, "antonym", &gel_id).unwrap();
    let out = canon_lift(&export_lift(&p).unwrap());
    let y = &out.iter().find(|e| e.id == "yüz_1").unwrap().relations;
    assert!(y.iter().any(|(_, a)| a.contains(&("type".to_string(), "Antonym".to_string())) && a.contains(&("ref".to_string(), "gel_8c3d".to_string()))), "{y:?}");
    let g = &out.iter().find(|e| e.id == "gel_8c3d").unwrap().relations;
    assert!(g.iter().any(|(_, a)| a.contains(&("ref".to_string(), "yüz_1".to_string()))), "the antonym is written on its other end too: {g:?}");
    fs::remove_dir_all(root).ok();
}

#[test]
fn lift_report_names_what_is_not_carried() {
    let (root, mut p) = temp_project();
    let r = import_lift(&mut p, LIFT).unwrap();
    assert_eq!(r.spans_flattened, 1, "one span flattened");
    let nc = &r.not_carried.0;
    assert_eq!(nc.get("example/note"), Some(&1), "an example's note counted: {nc:?}");
    assert_eq!(nc.get("entry/note"), None, "an entry's note is carried: {nc:?}");
    assert_eq!(r.notes, 1);
    assert_eq!(nc.get("header"), Some(&1), "header counted: {nc:?}");
    assert_eq!(nc.get("form/annotation"), Some(&1), "form annotation counted: {nc:?}");
    assert_eq!(r.not_carried.total(), 3, "nothing else dropped: {nc:?}");
    fs::remove_dir_all(root).ok();
}

#[test]
fn lift_notes_come_in_and_go_back_out_with_edits() {
    let (root, mut p) = temp_project();
    import_lift(&mut p, LIFT).unwrap();
    let lexemes = p.proj.entities_of_kind("lexeme").unwrap();
    let yuz2 = lexemes.iter().find(|l| p.proj.field_str(l, "x.lift.id").unwrap().as_deref() == Some("yüz_2")).unwrap().clone();
    assert_eq!(p.proj.field_str(&yuz2, "notes").unwrap().as_deref(), Some("homograph of yüz 'face'"));
    let flat = |x: &str| x.lines().map(str::trim).collect::<String>();
    let out = flat(&export_lift(&p).unwrap());
    assert!(out.contains("<note><form lang=\"en\"><text>homograph of yüz 'face'</text></form></note>") || out.contains("<note><form lang=\"en\"><text>homograph of yüz &apos;face&apos;</text></form></note>"), "{out}");

    // Edited in wugs, the note goes out as it reads now; a sense note typed in wugs goes out too.
    p.set_lexeme_notes(&yuz2, "not the same word as yüz 'face'").unwrap();
    let sense = p.proj.ordered_children("lexeme", &yuz2, "sense").unwrap()[0].clone();
    p.set_sense_text(&sense, "notes", "ask about the causative").unwrap();
    let out = flat(&export_lift(&p).unwrap());
    assert!(!out.contains("homograph of"), "{out}");
    assert!(out.contains("not the same word as yüz"), "{out}");
    assert!(out.contains("<note><form lang=\"en\"><text>ask about the causative</text></form></note>"), "{out}");

    // Cleared, it is left out; and the export reads back unchanged.
    p.set_lexeme_notes(&yuz2, "").unwrap();
    let out = export_lift(&p).unwrap();
    assert!(!out.contains("not the same word"), "{out}");
    let (root2, mut q) = temp_project();
    let r = import_lift(&mut q, &out).unwrap();
    assert_eq!(r.notes, 1, "the sense's note");
    assert_eq!(export_lift(&q).unwrap(), out);
    fs::remove_dir_all(root).ok();
    fs::remove_dir_all(root2).ok();
}

#[test]
fn lift_reimport_is_idempotent() {
    let (root, mut p) = temp_project();
    import_lift(&mut p, LIFT).unwrap();
    let before = counts(&p);
    let again = import_lift(&mut p, LIFT).unwrap();
    assert_eq!((again.entries_created, again.entries_skipped), (0, 6), "original re-import creates nothing");
    let exported = export_lift(&p).unwrap();
    let own = import_lift(&mut p, &exported).unwrap();
    assert_eq!(own.entries_created, 0, "own export re-import creates nothing");
    assert_eq!(counts(&p), before, "entity, field and op counts unchanged");
    fs::remove_dir_all(root).ok();
}

#[test]
fn flextext_round_trip_is_semantically_identical() {
    let (root, mut p) = temp_project();
    import_lift(&mut p, LIFT).unwrap();
    let r = import_flextext(&mut p, FLEXTEXT, "fallback").unwrap();
    assert_eq!(
        (r.texts_created, r.phrases_created, r.tokens_created, r.morphs_created),
        (1, 3, 7, 7),
        "everything imported once"
    );
    let txt = &r.texts[0];
    assert_eq!(p.proj.field_str(txt, "title").unwrap().unwrap(), "Dogs came", "title from the title item");
    assert_eq!(
        canon_flextext(&export_flextext(&p, txt).unwrap()),
        canon_flextext(FLEXTEXT),
        "exported FlexText matches the original"
    );
    assert_eq!(r.findings, 0, "imported glossing passes the validators");
    fs::remove_dir_all(root).ok();
}

#[test]
fn morphs_link_to_the_unique_matching_sense() {
    let (root, mut p) = temp_project();
    import_lift(&mut p, LIFT).unwrap();
    let r = import_flextext(&mut p, FLEXTEXT, "T").unwrap();
    assert_eq!((r.senses_linked, r.links_ambiguous, r.links_unmatched), (7, 0, 0), "all seven glossed morphs link");
    let yuz = p
        .proj
        .children_by_ref("text", &r.texts[0], "segment")
        .unwrap()
        .into_iter()
        .flat_map(|s| p.proj.children_by_ref("segment", &s, "token").unwrap())
        .flat_map(|t| p.proj.children_by_ref("token", &t, "morph").unwrap())
        .find(|m| p.proj.field_str(m, "surface").unwrap().as_deref() == Some("yüz"))
        .unwrap();
    let sense = p.proj.field_str(&yuz, "sense").unwrap().unwrap();
    assert_eq!(
        p.proj.field_str(&sense, "origin").unwrap().as_deref(),
        Some("yüz2_s1"),
        "homograph resolved by gloss: yüz 'hundred', not yüz 'face'"
    );

    let (root2, mut bare) = temp_project();
    let r2 = import_flextext(&mut bare, FLEXTEXT, "T").unwrap();
    assert_eq!((r2.senses_linked, r2.links_unmatched), (0, 7), "no lexicon, no links, nothing invented");
    fs::remove_dir_all(root).ok();
    fs::remove_dir_all(root2).ok();
}

#[test]
fn flextext_reimport_and_replay() {
    let (root, mut p) = temp_project();
    let first = import_flextext(&mut p, FLEXTEXT, "T").unwrap();
    let before = counts(&p);
    let again = import_flextext(&mut p, FLEXTEXT, "T").unwrap();
    assert_eq!(again.texts, first.texts, "same file resolves to the same text");
    assert_eq!((again.phrases_created, again.phrases_skipped), (0, 3), "no duplicate phrases");
    let exported = export_flextext(&p, &first.texts[0]).unwrap();
    let own = import_flextext(&mut p, &exported, "T").unwrap();
    assert_eq!(own.texts, first.texts, "own export resolves to the same text");
    assert_eq!(counts(&p), before, "counts unchanged after both re-imports");

    drop(p);
    fs::remove_dir_all(root.join("cache")).unwrap();
    let p2 = Project::open(&root, "prs_Tester").unwrap();
    assert_eq!(
        canon_flextext(&export_flextext(&p2, &first.texts[0]).unwrap()),
        canon_flextext(FLEXTEXT),
        "export from the replayed projection matches"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn edits_made_in_wugs_reach_the_export() {
    let (root, mut p) = temp_project();
    let r = import_flextext(&mut p, FLEXTEXT, "T").unwrap();
    let txt = r.texts[0].clone();
    let morph = p
        .proj
        .children_by_ref("text", &txt, "segment")
        .unwrap()
        .into_iter()
        .flat_map(|s| p.proj.children_by_ref("segment", &s, "token").unwrap())
        .flat_map(|t| p.proj.children_by_ref("token", &t, "morph").unwrap())
        .find(|m| p.proj.field_str(m, "gloss").unwrap().as_deref() == Some("PST"))
        .unwrap();
    p.set_gloss(&txt, &morph, "PFV", None).unwrap();
    let out = export_flextext(&p, &txt).unwrap();
    assert!(out.contains(">PFV</item>") && !out.contains(">PST</item>"), "edited gloss replaces the imported one");
    fs::remove_dir_all(root).ok();
}

#[test]
fn invalid_input_is_rejected_without_writes() {
    let (root, mut p) = temp_project();
    let before = counts(&p);
    for bad in [
        "<lift>",
        "<notlift/>",
        "<lift><entry/></lift>",
        "<lift><entry id=\"a\"><lexical-unit/></entry></lift>",
        "<lift><entry id=\"a\" order=\"x\"><lexical-unit><form lang=\"tr\"><text>a</text></form></lexical-unit></entry></lift>",
        "<lift><entry id=\"a\"><lexical-unit><form lang=\"tr\"><text>a</text></form></lexical-unit></entry><entry id=\"a\"><lexical-unit><form lang=\"tr\"><text>a</text></form></lexical-unit></entry></lift>",
    ] {
        assert!(import_lift(&mut p, bad).is_err(), "must reject LIFT {bad}");
    }
    for bad in [
        "<document>",
        "<interlinear-text/>",
        "<document><interlinear-text><paragraphs><paragraph><phrases><phrase begin-time-offset=\"soon\"/></phrases></paragraph></paragraphs></interlinear-text></document>",
        "<document><interlinear-text><item lang=\"en\">no type</item></interlinear-text></document>",
        "<document><interlinear-text guid=\"g\"><paragraphs><paragraph><phrases><phrase guid=\"g\"/></phrases></paragraph></paragraphs></interlinear-text></document>",
    ] {
        assert!(import_flextext(&mut p, bad, "T").is_err(), "must reject FlexText {bad}");
    }
    assert_eq!(counts(&p), before, "rejected input leaves the project untouched");
    fs::remove_dir_all(root).ok();
}

#[test]
fn every_import_txn_stays_in_one_scope() {
    let (root, mut p) = temp_project();
    import_lift(&mut p, LIFT).unwrap();
    import_flextext(&mut p, FLEXTEXT, "T").unwrap();
    let mut scopes: std::collections::HashMap<String, std::collections::HashSet<String>> = Default::default();
    for op in p.log.read_all().unwrap().ops {
        scopes.entry(op.txn).or_default().insert(op.scope);
    }
    assert!(scopes.len() > 3, "several txns written");
    for (txn, s) in &scopes {
        assert_eq!(s.len(), 1, "txn {txn} spans scopes {s:?}, which scope-filtered sync would split");
    }
    fs::remove_dir_all(root).ok();
}

fn morphs_of(p: &Project, text: &str) -> Vec<String> {
    p.proj
        .children_by_ref("text", text, "segment")
        .unwrap()
        .into_iter()
        .flat_map(|s| p.proj.children_by_ref("segment", &s, "token").unwrap())
        .flat_map(|t| p.proj.children_by_ref("token", &t, "morph").unwrap())
        .collect()
}

#[test]
fn repeated_analysis_guids_import() {
    let (root, mut p) = temp_project();
    let r = import_flextext(&mut p, FLEXTEXT, "T").unwrap();
    let kopek = morphs_of(&p, &r.texts[0])
        .into_iter()
        .filter(|m| p.proj.field_str(m, "surface").unwrap().as_deref() == Some("köpek"))
        .count();
    assert_eq!(kopek, 2, "the same analysis guid used twice gives two morph occurrences");
    fs::remove_dir_all(root).ok();
}

#[test]
fn guidless_file_and_born_digital_text_reimport_onto_themselves() {
    let (root, mut p) = temp_project();
    let stripped: String = FLEXTEXT
        .lines()
        .map(|l| match l.find(" guid=\"") {
            Some(i) => {
                let end = l[i + 7..].find('"').unwrap() + i + 8;
                format!("{}{}", &l[..i], &l[end..])
            }
            None => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!stripped.contains("guid=\"c0de"), "fixture stripped of guids");
    let first = import_flextext(&mut p, &stripped, "T").unwrap();
    let before = counts(&p);
    let exported = export_flextext(&p, &first.texts[0]).unwrap();
    let again = import_flextext(&mut p, &exported, "T").unwrap();
    assert_eq!(again.texts, first.texts, "export of a guid-less import resolves to the same text");
    assert_eq!(again.phrases_created, 0, "no duplicate phrases");
    assert_eq!(counts(&p), before, "counts unchanged");

    let mut t = p.intent("create-tier");
    let tier = t.create("tier", "tir", wugs_core::SCOPE_CONFIG, vec![("name", wugs_model::Value::str("utt"))]);
    t.commit().unwrap();
    let txt = p.create_text("Born digital").unwrap();
    p.add_segment(&txt, &tier, 0, Some(0.0), Some(900.0), "köpekler geldi").unwrap();
    let before = counts(&p);
    let born = export_flextext(&p, &txt).unwrap();
    let r = import_flextext(&mut p, &born, "T").unwrap();
    assert_eq!((r.texts.clone(), r.phrases_created), (vec![txt], 0), "born-digital export resolves to itself");
    assert_eq!(counts(&p), before, "counts unchanged");
    fs::remove_dir_all(root).ok();
}

#[test]
fn type_changes_and_cleared_values_reach_the_export() {
    let (root, mut p) = temp_project();
    let r = import_flextext(&mut p, FLEXTEXT, "T").unwrap();
    let txt = r.texts[0].clone();
    let di = morphs_of(&p, &txt)
        .into_iter()
        .find(|m| p.proj.field_str(m, "surface").unwrap().as_deref() == Some("-di"))
        .unwrap();
    let mut t = p.intent("edit-morph");
    t.set("morph", &di, &txt, "type", wugs_model::Value::str("clitic")).unwrap();
    t.commit().unwrap();
    p.set_gloss(&txt, &di, "", None).unwrap();
    let free = p
        .proj
        .children_by_ref("text", &txt, "segment")
        .unwrap()
        .into_iter()
        .find(|s| p.proj.field_str(s, "content").unwrap().as_deref() == Some("a hundred; Ali & Ayşe"))
        .unwrap();
    let mut t = p.intent("delete-translation");
    t.delete("segment", &free, &txt);
    t.commit().unwrap();

    let out = export_flextext(&p, &txt).unwrap();
    assert!(out.contains("type=\"clitic\""), "edited morph type exported");
    assert!(!out.contains(">PST</item>"), "cleared gloss not resurrected from stored items");
    assert!(!out.contains("a hundred; Ali"), "deleted free translation not resurrected");
    assert!(out.contains("Die Hunde kamen."), "untouched items stay");
    fs::remove_dir_all(root).ok();
}

#[test]
fn reanalysing_an_imported_word_keeps_its_flex_identity() {
    let (root, mut p) = temp_project();
    import_lift(&mut p, LIFT).unwrap();
    let txt = import_flextext(&mut p, FLEXTEXT, "T").unwrap().texts[0].clone();
    let word = p
        .proj
        .children_by_ref("text", &txt, "segment")
        .unwrap()
        .into_iter()
        .flat_map(|s| p.proj.members(&s).unwrap())
        .find(|t| p.proj.field_str(t, "x.flex.guid").unwrap().as_deref() == Some("c0de0001-0000-4000-8000-0000000000w1"))
        .unwrap();
    let morphs = p.proj.members(&word).unwrap();
    let before: Vec<_> = morphs.iter().map(|m| p.proj.fields_of(m).unwrap()).collect();
    let mut specs: Vec<wugs_core::MorphSpec> = morphs.iter().map(|m| p.morph_spec(m).unwrap()).collect();
    specs[1].gloss = Some("PL.NOM".into());

    let (ids, _) = p.apply_analysis(&txt, &word, &specs).unwrap();
    assert_eq!(ids, morphs, "the same morphs, re-glossed in place");
    for (m, had) in morphs.iter().zip(&before) {
        for (field, value) in had.iter().filter(|(f, _)| f.starts_with("x.flex.") || f == "form" || f == "sense") {
            assert_eq!(p.proj.field(m, field).unwrap().map(|(v, _)| v).as_ref(), Some(value), "{m}.{field} kept");
        }
    }
    let out = export_flextext(&p, &txt).unwrap();
    assert!(out.contains("guid=\"c0de0001-0000-4000-8000-0000000000m1\"") && out.contains("guid=\"c0de0001-0000-4000-8000-0000000000m2\""), "morph guids survive to the export");
    assert!(out.contains(">PL.NOM</item>"), "the new gloss is exported");
    fs::remove_dir_all(root).ok();
}

#[test]
fn an_allomorph_added_in_wugs_goes_out_as_a_variant() {
    let (root, mut p) = temp_project();
    let ((lex, _), _) = p.add_lexeme("-ler", "PL").unwrap();
    p.add_allomorph(&lex, "-lar").unwrap();
    let out = export_lift(&p).unwrap();
    let doc = xml::parse(&out).unwrap();
    let entry = doc.children_named("entry").next().unwrap();
    let variants: Vec<Multi> = entry.children_named("variant").map(|v| forms(Some(v))).collect();
    assert_eq!(variants.len(), 1, "{out}");
    assert!(format!("{variants:?}").contains("-lar"), "{out}");
    // Read back, it is an other form of the same entry.
    let (root2, mut q) = temp_project();
    import_lift(&mut q, &out).unwrap();
    let lex2 = q.proj.entities_of_kind("lexeme").unwrap()[0].clone();
    assert_eq!(q.allomorphs(&lex2).unwrap().iter().map(|(_, f)| f.as_str()).collect::<Vec<_>>(), ["-lar"]);
    fs::remove_dir_all(root).ok();
    fs::remove_dir_all(root2).ok();
}

#[test]
fn flextext_notes_become_notes_and_go_back_out() {
    let (root, mut p) = temp_project();
    let r = import_flextext(&mut p, FLEXTEXT, "T").unwrap();
    let txt = r.texts[0].clone();
    let seg = p.proj.ordered_children("text", &txt, "segment").unwrap().into_iter().find(|s| !p.segment_notes(s).unwrap().is_empty()).expect("the note became a note");
    let notes = p.segment_notes(&seg).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].1, "Said quickly; check the vowel of \"came\".");
    // Unchanged, it goes out as it came (the round trip test checks the rest).
    let out = export_flextext(&p, &txt).unwrap();
    assert_eq!(out.matches("type=\"note\"").count(), 1, "{out}");
    assert!(out.contains("check the vowel of"));
    // Edited and added in wugs, the export follows.
    p.set_note(&txt, &notes[0].0, "Said quickly.").unwrap();
    p.add_segment_note(&txt, &seg, "Second take is clearer.").unwrap();
    let out = export_flextext(&p, &txt).unwrap();
    assert!(out.contains("<item type=\"note\" lang=\"en\">Said quickly.</item>") && out.contains("<item type=\"note\" lang=\"en\">Second take is clearer.</item>"), "{out}");
    assert!(!out.contains("check the vowel"));
    for (id, _) in p.segment_notes(&seg).unwrap() {
        p.delete_note(&txt, &id).unwrap();
    }
    assert!(!export_flextext(&p, &txt).unwrap().contains("type=\"note\""), "deleted notes leave");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn illustrations_wait_for_their_picture_and_go_back_out() {
    let lift = r#"<?xml version="1.0" encoding="UTF-8"?>
<lift version="0.13">
  <entry id="kesi_1" guid="11111111-1111-1111-1111-111111111111">
    <lexical-unit><form lang="kan"><text>kesi</text></form></lexical-unit>
    <sense id="kesi_s1">
      <gloss lang="en"><text>fox</text></gloss>
      <illustration href="fox.jpg"><label><form lang="en"><text>a red fox</text></form></label></illustration>
      <illustration href="den.png"/>
    </sense>
  </entry>
</lift>"#;
    let (root, mut p) = temp_project();
    let r = import_lift(&mut p, lift).unwrap();
    assert_eq!(r.not_carried.total(), 0, "{:?}", r.not_carried);
    assert_eq!(r.illustrations.len(), 2);
    let (sense, href) = r.illustrations[0].clone();
    assert_eq!(href, "fox.jpg");
    // Not found yet: both go back out as they came.
    let out = export_lift(&p).unwrap();
    assert!(out.contains(r#"<illustration href="fox.jpg">"#) && out.contains("a red fox") && out.contains(r#"<illustration href="den.png"/>"#), "{out}");
    // Found beside the file: it becomes the picture, its label kept, written once.
    let blob = format!("blake3:{}", "cd".repeat(32));
    wugs_flex::attach_illustration(&mut p, &sense, "fox.jpg", &blob, "fox.jpg").unwrap();
    assert_eq!(p.sense_picture(&sense).unwrap(), Some((blob, "fox.jpg".to_string())));
    let out = export_lift(&p).unwrap();
    assert_eq!(out.matches(r#"href="fox.jpg""#).count(), 1, "{out}");
    assert!(out.contains("a red fox") && out.contains(r#"href="den.png""#), "{out}");
    // Taken off the sense in wugs: it no longer goes out.
    p.set_sense_picture(&sense, None).unwrap();
    let out = export_lift(&p).unwrap();
    assert!(!out.contains("fox.jpg") && out.contains("den.png"), "{out}");
    fs::remove_dir_all(root).ok();
}
