//! Import -> export -> diff harness for Toolbox MDF lexicons.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use wugs_core::Project;
use wugs_legacy::{export_toolbox, import_toolbox};
use wugs_model::Value;

const DB: &str = include_str!("fixtures/sample.db");

fn temp_project() -> (PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("wugs-legacy-tbx-{}", wugs_model::ulid()));
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

/// An SFM file read straight from text: header lines, then each `\lx`
/// record as its ordered (marker, value) lines. Line order inside a record
/// carries MDF meaning (which sense a line belongs to), so it is compared.
#[derive(Debug, PartialEq)]
struct Canon {
    header: Vec<String>,
    records: Vec<Vec<(String, String)>>,
}

fn canon(s: &str) -> Canon {
    let mut header = Vec::new();
    let mut records: Vec<Vec<(String, String)>> = Vec::new();
    for line in s.trim_start_matches('\u{feff}').split('\n').map(|l| l.trim_end_matches('\r')) {
        if let Some(body) = line.strip_prefix('\\') {
            let (m, v) = body.split_once(' ').unwrap_or((body, ""));
            if m == "lx" {
                records.push(Vec::new());
            }
            match records.last_mut() {
                Some(r) => r.push((m.to_string(), v.trim().to_string())),
                None => header.push(line.to_string()),
            }
        } else if let Some((_, v)) = records.last_mut().and_then(|r| r.last_mut()) {
            v.push('\n');
            v.push_str(line.trim_end());
        }
    }
    for (_, v) in records.iter_mut().flatten() {
        *v = v.trim().to_string();
    }
    Canon { header, records }
}

fn lexeme(p: &Project, lemma: &str, hm: Option<f64>) -> String {
    p.proj
        .entities_of_kind("lexeme")
        .unwrap()
        .into_iter()
        .find(|l| {
            p.proj.field_str(l, "lemma").unwrap().as_deref() == Some(lemma) && p.proj.field_num(l, "homograph").unwrap() == hm
        })
        .unwrap()
}

fn senses(p: &Project, lex: &str) -> Vec<String> {
    p.proj.ordered_children("lexeme", lex, "sense").unwrap()
}

fn s(p: &Project, id: &str, f: &str) -> Option<String> {
    p.proj.field(id, f).unwrap().and_then(|(v, _)| v.as_str().map(str::to_string))
}

#[test]
fn fixture_reads_as_intended() {
    let c = canon(DB);
    assert_eq!(c.header, vec!["\\_sh v3.0  400  MDF 4.0", "\\_DateStampHasFourDigitYear"]);
    assert_eq!(c.records.len(), 7, "seven records");
    assert_eq!(c.records[1][4].1, "body part;\ncontinues on a second line", "continuation folded");
}

#[test]
fn round_trip_is_identical() {
    let (root, mut p) = temp_project();
    let r = import_toolbox(&mut p, DB).unwrap();
    assert_eq!((r.entries_created, r.senses_created, r.variants_created, r.examples_created), (7, 9, 2, 2), "everything imported once");
    assert_eq!(canon(&export_toolbox(&p).unwrap()), canon(DB), "export has every line, in place");
    fs::remove_dir_all(root).ok();
}

#[test]
fn report_counts_every_verbatim_line() {
    let (root, mut p) = temp_project();
    let r = import_toolbox(&mut p, DB).unwrap();
    let want: BTreeMap<String, usize> =
        [("cf", 1), ("dt", 5), ("ge", 1), ("gn", 1), ("nt", 2)].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    assert_eq!(r.markers_kept_verbatim.0, want, "uncarried markers, plus the repeated \\ge, counted by marker");
    assert_eq!(r.duplicate_headwords, 1, "second bank without \\hm reported");
    fs::remove_dir_all(root).ok();
}

#[test]
fn mdf_markers_map_to_the_lexicon() {
    let (root, mut p) = temp_project();
    import_toolbox(&mut p, DB).unwrap();

    let kopek = lexeme(&p, "köpek", None);
    let ks = senses(&p, &kopek);
    assert_eq!(ks.len(), 1, "no \\sn: one sense");
    assert_eq!(s(&p, &ks[0], "gloss").as_deref(), Some("dog"));
    assert_eq!(s(&p, &ks[0], "definition").as_deref(), Some("a domesticated canine"));
    assert_eq!(s(&p, &ks[0], "x.toolbox.ps").as_deref(), Some("n"));

    let hundred = lexeme(&p, "yüz", Some(2.0));
    assert_eq!(s(&p, &senses(&p, &hundred)[0], "gloss").as_deref(), Some("hundred"), "homographs kept apart");

    let gel = lexeme(&p, "gel", None);
    assert_eq!(s(&p, &gel, "citation").as_deref(), Some("gelmek"), "\\lc is the citation form");
    let gs = senses(&p, &gel);
    let view: Vec<_> = gs.iter().map(|x| (s(&p, x, "x.toolbox.ps"), s(&p, x, "gloss"))).collect();
    assert_eq!(
        view,
        vec![
            (Some("v".into()), Some("come".into())),
            (None, Some("arrive".into())),
            (Some("adv".into()), Some("arrivingly".into())),
        ],
        "\\sn opens senses; a \\ps right above \\sn belongs to that sense"
    );
    let forms: Vec<_> = ["köpek", "gel"]
        .iter()
        .flat_map(|l| p.proj.ordered_children("lexeme", &lexeme(&p, l, None), "form").unwrap())
        .map(|f| (s(&p, &f, "kind"), s(&p, &f, "form")))
        .collect();
    assert_eq!(
        forms,
        vec![(Some("variant".into()), Some("köpük".into())), (Some("variant".into()), Some("gäl".into()))],
        "\\va are variant forms"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn reimport_is_idempotent() {
    let (root, mut p) = temp_project();
    import_toolbox(&mut p, DB).unwrap();
    let before = counts(&p);
    let again = import_toolbox(&mut p, DB).unwrap();
    assert_eq!((again.entries_created, again.entries_skipped), (0, 7), "original re-import creates nothing");
    let exported = export_toolbox(&p).unwrap();
    let own = import_toolbox(&mut p, &exported).unwrap();
    assert_eq!(own.entries_created, 0, "own export re-import creates nothing");
    assert_eq!(counts(&p), before, "entity, field and op counts unchanged");
    fs::remove_dir_all(root).ok();
}

#[test]
fn export_from_replayed_log_matches() {
    let (root, mut p) = temp_project();
    import_toolbox(&mut p, DB).unwrap();
    drop(p);
    fs::remove_dir_all(root.join("cache")).unwrap();
    let p2 = Project::open(&root, "prs_Tester").unwrap();
    assert_eq!(canon(&export_toolbox(&p2).unwrap()), canon(DB), "export from the replayed projection matches");
    fs::remove_dir_all(root).ok();
}

#[test]
fn edits_made_in_wugs_reach_the_export() {
    let (root, mut p) = temp_project();
    import_toolbox(&mut p, DB).unwrap();
    let kopek = lexeme(&p, "köpek", None);
    let ks = senses(&p, &kopek)[0].clone();
    let gel = lexeme(&p, "gel", None);
    let gs = senses(&p, &gel);
    let mut t = p.intent("edit");
    t.set("sense", &ks, "lexicon", "gloss", Value::str("hound")).unwrap();
    t.set("sense", &ks, "lexicon", "definition", Value::Null).unwrap();
    t.delete("sense", &gs[1], "lexicon");
    t.commit().unwrap();
    let (born, _) = p.create_lexeme("kedi", "cat").unwrap();

    let out = canon(&export_toolbox(&p).unwrap());
    let rec = |lx: &str| out.records.iter().find(|r| r[0].1 == lx).unwrap().clone();
    let k = rec("köpek");
    assert!(k.contains(&("ge".into(), "hound".into())), "edited gloss exported: {k:?}");
    assert!(!k.iter().any(|(m, _)| m == "de"), "cleared definition not resurrected: {k:?}");
    assert_eq!(k.iter().position(|(m, _)| m == "gn"), Some(3), "verbatim lines keep their place: {k:?}");
    let g: Vec<String> = rec("gel").into_iter().map(|(m, v)| format!("{m} {v}")).collect();
    assert_eq!(
        g,
        ["lx gel", "lc gelmek", "ps v", "sn 1", "ge come", "xv Geldi.", "xe He came.", "sn 3", "ge arrivingly", "va gäl", "dt 02/Feb/2006", "ps adv"],
        "deleted sense takes its own lines; the \\ps it held for sense 3 moves to the end of sense 3"
    );
    assert_eq!(rec("kedi"), vec![("lx".into(), "kedi".into()), ("ge".into(), "cat".into())], "born-digital entry");

    let before = counts(&p);
    let exported = export_toolbox(&p).unwrap();
    let r = import_toolbox(&mut p, &exported).unwrap();
    assert_eq!(r.entries_created, 0, "edited and born-digital entries re-import onto themselves");
    assert_eq!(counts(&p), before);
    assert_eq!(s(&p, &born, "lemma").as_deref(), Some("kedi"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn examples_come_in_with_their_sense_and_go_back_out() {
    let (root, mut p) = temp_project();
    import_toolbox(&mut p, DB).unwrap();
    let kopek = lexeme(&p, "köpek", None);
    let ks = senses(&p, &kopek)[0].clone();
    let ex = p.examples(&ks).unwrap();
    assert_eq!((ex.len(), ex[0].sentence.as_str(), ex[0].translation.as_str()), (1, "Köpek havladı.", "The dog barked."));
    let gel = lexeme(&p, "gel", None);
    assert_eq!(p.examples(&senses(&p, &gel)[0]).unwrap()[0].sentence, "Geldi.", "an example under \\sn 1 belongs to sense 1");
    // Edited in wugs: the line keeps its place; a new example follows the sense's lines.
    p.set_example_text(&ex[0].id, "translation", "The dog was barking.").unwrap();
    p.add_example(&ks, "Köpek koştu.", "The dog ran.", None, None).unwrap();
    p.add_example(&ks, "Köpek!", "", None, None).unwrap();
    let out = canon(&export_toolbox(&p).unwrap());
    let k: Vec<String> = out.records.iter().find(|r| r[0].1 == "köpek").unwrap().iter().map(|(m, v)| format!("{m} {v}")).collect();
    assert_eq!(
        k,
        ["lx köpek", "ps n", "ge dog", "de a domesticated canine", "gn chien", "xv Köpek havladı.", "xe The dog was barking.", "va köpük", "dt 12/Mar/2004", "xv Köpek koştu.", "xe The dog ran.", "xv Köpek!"],
        "no empty \\xe for an example without a translation"
    );
    let before = counts(&p);
    let exported = export_toolbox(&p).unwrap();
    let r = import_toolbox(&mut p, &exported).unwrap();
    assert_eq!(r.entries_created, 0);
    assert_eq!(counts(&p), before, "our own export re-imports onto itself");
    fs::remove_dir_all(root).ok();
}

#[test]
fn invalid_input_is_rejected_without_writes() {
    let (root, mut p) = temp_project();
    let before = counts(&p);
    for bad in [
        "",
        "just some prose",
        "\\_sh v3.0  400  MDF 4.0\n\\ge orphan line outside a record",
        "\\lx\n\\ge empty headword",
        "\\lx a\n\\hm two",
        "\\lx a\n\\ no marker",
        // a valid record first: nothing of it may be written either
        "\\lx ok\n\\ge fine\n\n\\lx b\n\\hm x",
    ] {
        assert!(import_toolbox(&mut p, bad).is_err(), "must reject {bad:?}");
    }
    assert_eq!(counts(&p), before, "rejected input leaves the project untouched");
    fs::remove_dir_all(root).ok();
}

#[test]
fn every_import_txn_stays_in_one_scope() {
    let (root, mut p) = temp_project();
    import_toolbox(&mut p, DB).unwrap();
    let mut scopes: BTreeMap<String, std::collections::BTreeSet<String>> = Default::default();
    for op in p.log.read_all().unwrap().ops {
        scopes.entry(op.txn).or_default().insert(op.scope);
    }
    assert_eq!(scopes.len(), 8, "one txn per entry plus the header source");
    for (txn, s) in &scopes {
        assert_eq!(s.len(), 1, "txn {txn} spans scopes {s:?}");
    }
    fs::remove_dir_all(root).ok();
}

#[test]
fn bom_and_crlf_are_tolerated() {
    let (root, mut p) = temp_project();
    let crlf = format!("\u{feff}{}", DB.replace('\n', "\r\n"));
    import_toolbox(&mut p, &crlf).unwrap();
    assert_eq!(canon(&export_toolbox(&p).unwrap()), canon(DB));
    fs::remove_dir_all(root).ok();
}

#[test]
fn own_export_of_a_headerless_file_reimports_as_a_no_op() {
    let (root, mut p) = temp_project();
    let headerless = "\\lx kedi\n\\ge cat\n\n\\lx it\n\\ge dog\n";
    import_toolbox(&mut p, headerless).unwrap();
    let exported = export_toolbox(&p).unwrap();
    assert!(exported.starts_with(wugs_legacy::toolbox::DEFAULT_HEADER), "export supplies the default header");
    let before = counts(&p);
    let r = import_toolbox(&mut p, &exported).unwrap();
    assert_eq!((r.entries_created, r.commits), (0, 0), "nothing written");
    assert_eq!(counts(&p), before, "no phantom header source");
    assert!(p.proj.entities_of_kind("source").unwrap().is_empty());
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_record_shadowed_by_a_born_digital_headword_is_counted_apart() {
    let (root, mut p) = temp_project();
    p.create_lexeme("kedi", "cat").unwrap();
    let colleague = "\\lx kedi\n\\ps n\n\\ge cat\n\\de a small domesticated feline\n\\nt from the 2019 survey\n";
    let r = import_toolbox(&mut p, colleague).unwrap();
    assert_eq!(
        (r.entries_created, r.entries_skipped, r.entries_skipped_matched_by_headword),
        (0, 0, 1),
        "the colleague's record is skipped, and the report says it matched by headword only"
    );
    import_toolbox(&mut p, DB).unwrap();
    let again = import_toolbox(&mut p, DB).unwrap();
    assert_eq!(
        (again.entries_skipped, again.entries_skipped_matched_by_headword),
        (7, 0),
        "origin matches are not headword matches"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn homograph_numbers_export_as_written() {
    let (root, mut p) = temp_project();
    let src = "\\_sh v3.0  400  MDF 4.0\n\n\\lx yüz\n\\hm 01\n\\ge face\n\n\\lx yüz\n\\hm 2\n\\ge hundred\n";
    import_toolbox(&mut p, src).unwrap();
    assert_eq!(canon(&export_toolbox(&p).unwrap()), canon(src), "\\hm 01 stays 01");
    let face = lexeme(&p, "yüz", Some(1.0));
    let mut t = p.intent("renumber");
    t.set("lexeme", &face, "lexicon", "homograph", Value::Num(3.0)).unwrap();
    t.commit().unwrap();
    let out = canon(&export_toolbox(&p).unwrap());
    assert_eq!(out.records[0][1], ("hm".into(), "3".into()), "an edited number wins over the written text");
    fs::remove_dir_all(root).ok();
}

#[test]
fn allomorphs_come_in_and_go_out_as_a_lines() {
    let (root, mut p) = temp_project();
    let db = "\\_sh v3.0  400  MDF 4.0\n\n\\lx -ler\n\\a -lar\n\\va -lér\n\\ps suf\n\\ge PL\n";
    let r = import_toolbox(&mut p, db).unwrap();
    assert_eq!(r.variants_created, 2);
    let lex = lexeme(&p, "-ler", None);
    let kinds: Vec<(Option<String>, Option<String>)> =
        p.proj.ordered_children("lexeme", &lex, "form").unwrap().iter().map(|f| (s(&p, f, "kind"), s(&p, f, "form"))).collect();
    assert_eq!(kinds, vec![(Some("allomorph".into()), Some("-lar".into())), (Some("variant".into()), Some("-lér".into()))]);
    assert!(!r.markers_kept_verbatim.0.contains_key("a"), "\\a is carried, not kept verbatim");
    assert_eq!(canon(&export_toolbox(&p).unwrap()), canon(db), "the lines go back where they were");
    // An allomorph added in wugs goes out as a new \a line.
    p.add_allomorph(&lex, "-lır").unwrap();
    let out = export_toolbox(&p).unwrap();
    assert!(out.contains("\\a -lır"), "{out}");
    fs::remove_dir_all(root).ok();
}

#[test]
fn pronunciations_and_relations_come_in_and_go_back_out() {
    let (root, mut p) = temp_project();
    let db = "\\_sh v3.0  400  MDF 4.0\n\n\\lx dakila\n\\ph daˈkila\n\\ps adj\n\\ge big\n\\sy laki\n\\an liit\n\\cf tela2\n\\cf nowhere\n\n\\lx laki\n\\ps adj\n\\ge large\n\\sy dakila\n\n\\lx liit\n\\ge small\n\n\\lx tela\n\\hm 1\n\\ge river\n\n\\lx tela\n\\hm 2\n\\ge wade\n";
    let r = import_toolbox(&mut p, db).unwrap();
    assert_eq!((r.entries_created, r.pronunciations_created, r.relations_created), (5, 1, 3), "the synonym written on both entries is one relation");
    assert_eq!(r.markers_kept_verbatim.0.get("cf"), Some(&1), "a \\cf naming no entry stays as written");
    let big = lexeme(&p, "dakila", None);
    assert_eq!(p.pronunciations(&big).unwrap()[0].form, "daˈkila");
    let mut rel: Vec<(String, String)> = p.relations(&big).unwrap().into_iter().map(|r| (r.relation, s(&p, &r.other, "lemma").unwrap())).collect();
    rel.sort();
    assert_eq!(rel, vec![("antonym".into(), "liit".into()), ("compare".into(), "tela".into()), ("synonym".into(), "laki".into())]);
    assert_eq!(p.relations(&lexeme(&p, "tela", Some(2.0))).unwrap().len(), 1, "\\cf tela2 names the second tela");
    assert_eq!(canon(&export_toolbox(&p).unwrap()), canon(db), "every line goes back where it was");
    // Renamed in wugs, the other end's line follows; a relation made in wugs gets its own line.
    p.set_lexeme_field(&lexeme(&p, "laki", None), "lemma", "lakí").unwrap();
    let small = lexeme(&p, "liit", None);
    let large = lexeme(&p, "lakí", None);
    p.add_relation(&small, "compare", &large).unwrap();
    let out = canon(&export_toolbox(&p).unwrap());
    assert!(out.records[0].contains(&("sy".to_string(), "lakí".to_string())), "{out:?}");
    assert!(out.records[2].contains(&("cf".to_string(), "lakí".to_string())), "{out:?}");
    // Importing it again adds nothing.
    let before = counts(&p);
    let again = import_toolbox(&mut p, db).unwrap();
    assert_eq!(again.relations_created, 0);
    assert_eq!(counts(&p).0, before.0);
    fs::remove_dir_all(root).ok();
}

const TEXT: &str = include_str!("fixtures/sample-text.txt");

#[test]
fn interlinear_texts_come_in_word_by_word() {
    use wugs_legacy::toolbox_text::{has_free_translation, import_toolbox_text, is_interlinear};
    assert!(is_interlinear(TEXT) && has_free_translation(TEXT));
    assert!(!is_interlinear(DB), "a lexicon is not a text");
    let (root, mut p) = temp_project();
    let (utt, _) = p.add_tier("utterance", "time-aligned", Some("transcription"), None).unwrap();
    let (tr, _) = p.add_translation_tier("translation", &utt).unwrap();
    // A root whose form and gloss match one sense is linked to it.
    let ((_, count), _) = p.add_lexeme("kani", "count").unwrap();
    let r = import_toolbox_text(&mut p, TEXT, "sample-text", &utt, Some(&tr)).unwrap();
    assert_eq!((r.texts.len(), r.sentences, r.words, r.morphs), (2, 3, 7, 9));
    assert_eq!((r.translations, r.notes, r.timed, r.linked), (2, 1, 1, 2));
    assert_eq!(r.not_carried.0.get("ps"), Some(&1), "{:?}", r.not_carried.0);
    let title = |t: &str| p.proj.field_str(t, "title").unwrap().unwrap();
    assert_eq!((title(&r.texts[0]).as_str(), title(&r.texts[1]).as_str()), ("Counting", "Market"));
    let segs: Vec<String> = p.proj.ordered_children("text", &r.texts[0], "segment").unwrap().into_iter().filter(|s| p.proj.field_str(s, "tier").unwrap().as_deref() == Some(utt.as_str())).collect();
    let content = |s: &str| p.proj.field_str(s, "content").unwrap().unwrap();
    assert_eq!(content(&segs[0]), "nikani ŋai");
    assert_eq!(content(&segs[1]), "kanima toru kesi", "the second bundle continues the sentence");
    assert_eq!((p.proj.field_num(&segs[0], "start").unwrap(), p.proj.field_num(&segs[0], "end").unwrap()), (Some(1250.0), Some(3900.0)));
    assert_eq!(p.segment_notes(&segs[0]).unwrap()[0].1, "Speaker counts on her fingers.");
    // Words and their morphs, markers turned into types.
    let toks = p.proj.members(&segs[1]).unwrap();
    let morphs: Vec<(String, String, String, Option<String>)> = p
        .proj
        .members(&toks[0])
        .unwrap()
        .iter()
        .map(|m| {
            let f = |k: &str| p.proj.field_str(m, k).unwrap().unwrap_or_default();
            (f("surface"), f("type"), f("gloss"), p.proj.field_str(m, "sense").unwrap())
        })
        .collect();
    assert_eq!(morphs, vec![("kani".into(), "root".into(), "count".into(), Some(count.clone())), ("ma".into(), "clitic".into(), "FOC".into(), None)]);
    // The translation pairs with its sentence.
    let tr_segs = p.proj.children_by_ref("parent", &segs[0], "segment").unwrap();
    assert_eq!(content(&tr_segs[0]), "I count one.");
    // Back out as Toolbox writes it, morphs and glosses under their words.
    let sentences: Vec<(String, Option<String>)> = segs.iter().map(|s| (s.clone(), p.proj.children_by_ref("parent", s, "segment").unwrap().first().map(|t| content(t)))).collect();
    let out = wugs_legacy::toolbox_text::export_toolbox_text(&p, "Counting", &sentences).unwrap();
    assert!(out.contains("\\ref Counting.001\n\\ELANBegin 00:00:01.250\n\\ELANEnd 00:00:03.900\n\\tx nikani     ŋai\n\\mb ni-  kani  ŋai\n\\ge 1SG- count one\n\\ft I count one.\n\\nt Speaker counts on her fingers.\n"), "{out}");
    assert!(out.contains("\\tx kanima     toru kesi\n\\mb kani  =ma  toru kesi\n\\ge count =FOC two  three\n"), "{out}");
    // A fresh project reads the export back to the same words, morphs and glosses.
    let (root2, mut q) = temp_project();
    let (utt2, _) = q.add_tier("utterance", "time-aligned", Some("transcription"), None).unwrap();
    let (tr2, _) = q.add_translation_tier("translation", &utt2).unwrap();
    let back = import_toolbox_text(&mut q, &out, "x", &utt2, Some(&tr2)).unwrap();
    assert_eq!((back.sentences, back.words, back.morphs, back.translations, back.notes, back.timed), (2, 5, 7, 2, 1, 1));
    fs::remove_dir_all(root2).ok();
    // The same file again adds nothing.
    let again = import_toolbox_text(&mut p, TEXT, "sample-text", &utt, Some(&tr)).unwrap();
    assert_eq!((again.texts.len(), again.matched.len()), (0, 2));
    fs::remove_dir_all(root).ok();
}
