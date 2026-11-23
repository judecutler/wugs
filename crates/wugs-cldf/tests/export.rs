//! A small glossed project exported as CLDF: contents checked here, and the
//! whole dataset checked by the reference validator (`cldf validate` from
//! pycldf) when it is installed.

use std::path::PathBuf;
use std::process::Command;

use wugs_cldf::{export, Dataset, Options};
use wugs_core::{concept_id, domain_id, ConceptRow, DomainRow, MorphSpec, Project};

fn temp_project() -> (PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("wugs-cldf-test-{}", wugs_model::ulid()));
    let p = Project::open(&root, "prs_Tester").unwrap();
    (root, p)
}

fn m(surface: &str, kind: &str, gloss: &str, sense: Option<&str>) -> MorphSpec {
    MorphSpec { surface: surface.into(), morph_type: kind.into(), gloss: Some(gloss.into()), glosses_by_lang: Default::default(), sense: sense.map(str::to_string) }
}

struct Built {
    root: PathBuf,
    p: Project,
    tiers: (Vec<String>, Vec<String>),
    count_sense: String,
}

/// Two texts (one restricted), each with a glossed sentence; a lexicon of
/// two entries with a Spanish gloss, a domain and a Concepticon link.
fn build() -> Built {
    let (root, mut p) = temp_project();
    p.set_project_language("Kanu", "abcd1234", "xyz").unwrap();
    let (utt, _) = p.add_tier("utterance", "time-aligned", Some("transcription"), None).unwrap();
    let (tr, _) = p.add_translation_tier("translation", &utt).unwrap();
    let mut t = p.intent("test-tier-language");
    t.set("tier", &tr, wugs_core::SCOPE_CONFIG, "language", wugs_model::Value::str("en")).unwrap();
    t.commit().unwrap();
    let ((count_lex, count), _) = p.add_lexeme("kani", "count").unwrap();
    p.set_lexeme_field(&count_lex, "pos-of-speech", "v").unwrap();
    p.set_sense_gloss(&count, "contar", Some("es")).unwrap();
    p.add_lexeme("teka", "ten").unwrap();
    p.import_semantic_domains("sil", &[DomainRow { code: "8".into(), guid: "00000000-0000-0000-0000-000000000001".into(), label: "Number".into(), description: String::new() }]).unwrap();
    p.assign_domain(&count, &domain_id("00000000-0000-0000-0000-000000000001").unwrap()).unwrap();
    p.import_concept_list("test:list", "Test list", "", &[ConceptRow { key: "k1".into(), number: 1, gloss: "count".into(), concepticon: "1234".into(), concepticon_gloss: "COUNT".into() }]).unwrap();
    p.link_concept(&count, &concept_id("test:list", "k1")).unwrap();

    for (title, restricted) in [("Counting, part 1", false), ("Private talk", true)] {
        let text = p.create_text(title).unwrap();
        if restricted {
            p.set_access(&text, "restricted").unwrap();
        }
        let (seg, _) = p.add_segment(&text, &utt, 0, Some(0.0), Some(1200.0), "nikani teka, \"ok\"").unwrap();
        p.add_translation(&text, &tr, &seg, 0, "I count ten, \"ok\"").unwrap();
        let (toks, _) = p.tokenize(&text, &seg, &["nikani", "teka", "ok"]).unwrap();
        p.apply_analysis(&text, &toks[0], &[m("ni-", "prefix", "1SG", None), m("kani", "root", "count", Some(&count))]).unwrap();
        p.apply_analysis(&text, &toks[1], &[m("teka", "root", "ten", None)]).unwrap();
        // An unglossed segment is not an example.
        p.add_segment(&text, &utt, 1, Some(1500.0), Some(2000.0), "toru").unwrap();
    }
    Built { root, p, tiers: (vec![utt], vec![tr]), count_sense: count }
}

fn run(b: &Built, include_restricted: bool) -> Dataset {
    export(&b.p, &Options { title: "Kanu dictionary", generator: "wugs test", transcription_tiers: &b.tiers.0, translation_tiers: &b.tiers.1, include_restricted }).unwrap()
}

fn file<'a>(d: &'a Dataset, name: &str) -> &'a str {
    &d.files.iter().find(|(n, _)| n == name).unwrap().1
}

#[test]
fn a_glossed_project_becomes_a_dictionary_with_examples() {
    let b = build();
    let d = run(&b, false);
    assert_eq!((d.entries, d.senses, d.examples, d.restricted_left_out), (2, 2, 1, 1));
    assert!(d.notes.iter().any(|n| n.contains("not open")), "{:?}", d.notes);
    let languages = file(&d, "languages.csv");
    assert!(languages.contains("abcd1234,Kanu,abcd1234,xyz"), "{languages}");
    assert!(languages.contains("meta-en,en,,"), "{languages}");
    let examples = file(&d, "examples.csv");
    assert!(examples.contains("\"nikani teka, \"\"ok\"\"\",ni-kani\tteka\tok,1SG-count\tten\t?,\"I count ten, \"\"ok\"\"\",meta-en,\"Counting, part 1\",0,1200"), "{examples}");
    assert!(!examples.contains("Private talk"));
    let senses = file(&d, "senses.csv");
    assert!(senses.starts_with("ID,Entry_ID,Description,Gloss_es,Definition,Usage,Semantic_Domains,Concepticon_ID,Concepticon_Gloss,Example_IDs\n"), "{senses}");
    let count_row = senses.lines().find(|l| l.starts_with(&b.count_sense)).unwrap();
    assert!(count_row.contains(",count,contar,,,8 Number,1234,COUNT,seg_"), "{count_row}");
    let entries = file(&d, "entries.csv");
    assert!(entries.lines().nth(1).unwrap().contains(",abcd1234,kani,v,"), "kani sorts before teka: {entries}");
    std::fs::remove_dir_all(&b.root).ok();
}

#[test]
fn restricted_texts_come_in_only_when_asked() {
    let b = build();
    let d = run(&b, true);
    assert_eq!((d.examples, d.restricted_left_out), (2, 0));
    assert!(file(&d, "README.md").contains("are included"));
    std::fs::remove_dir_all(&b.root).ok();
}

#[test]
fn the_reference_validator_accepts_the_dataset() {
    let b = build();
    let d = run(&b, true);
    let dir = b.root.join("cldf-out");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, contents) in &d.files {
        std::fs::write(dir.join(name), contents).unwrap();
    }
    match Command::new("cldf").arg("validate").arg(dir.join("Dictionary-metadata.json")).output() {
        Ok(out) => {
            let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            assert!(out.status.success() && !said.contains("WARNING") && !said.contains("ERROR"), "cldf validate:\n{said}");
        }
        Err(_) => eprintln!("pycldf is not installed (pip install pycldf); skipped the reference validator"),
    }
    // WUGS_KEEP_CLDF=1 keeps the folder to look at by hand.
    if std::env::var_os("WUGS_KEEP_CLDF").is_some() {
        eprintln!("kept {}", dir.display());
        return;
    }
    std::fs::remove_dir_all(&b.root).ok();
}
