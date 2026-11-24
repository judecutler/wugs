//! A small glossed project exported as CLDF: contents checked here, and the
//! whole dataset checked by the reference validator (`cldf validate` from
//! pycldf) when it is installed.

use std::path::PathBuf;
use std::process::Command;

use wugs_cldf::{export, export_structure, export_wordlist, structure, Dataset, Options, StructureOptions, WordlistOptions};
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
    p.add_example(&count, "kani toru", "count two", None, Some("Notebook 2")).unwrap();

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
    export(&b.p, &Options { title: "Kanu dictionary", generator: "wugs test", transcription_tiers: &b.tiers.0, translation_tiers: &b.tiers.1, include_restricted, audio: None }).unwrap()
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
    assert!(count_row.contains(",count,contar,,,8 Number,1234,COUNT,"), "{count_row}");
    // The sense lists its glossed sentence and the lexicon's own example.
    assert!(count_row.contains("exm_") && count_row.contains("seg_"), "{count_row}");
    assert_eq!(d.lexicon_examples, 1);
    assert!(examples.lines().any(|l| l.starts_with("exm_") && l.contains(",abcd1234,kani toru,,,count two,")), "{examples}");
    assert!(file(&d, "README.md").contains("1 example sentence the lexicon gives"), "{}", file(&d, "README.md"));
    let entries = file(&d, "entries.csv");
    assert!(entries.lines().nth(1).unwrap().contains(",abcd1234,kani,v,"), "kani sorts before teka: {entries}");
    assert!(!entries.starts_with("ID,Language_ID,Headword,Part_Of_Speech,Citation_Form,Homograph,Borrowed_From"), "no origin columns when no entry says: {entries}");
    std::fs::remove_dir_all(&b.root).ok();
}

#[test]
fn an_entrys_origin_goes_out_when_some_entry_has_one() {
    let mut b = build();
    let kani = b.p.proj.field_str(&b.count_sense, "lexeme").unwrap().unwrap();
    b.p.set_lexeme_field(&kani, "borrowed-from", "Malay").unwrap();
    b.p.set_lexeme_text(&kani, "etymology", "from Malay  kira 'count'").unwrap();
    let d = run(&b, false);
    let entries = file(&d, "entries.csv");
    assert!(entries.starts_with("ID,Language_ID,Headword,Part_Of_Speech,Citation_Form,Homograph,Borrowed_From,Etymology\n"), "{entries}");
    assert!(entries.lines().nth(1).unwrap().ends_with(",Malay,from Malay kira 'count'"), "{entries}");
    assert!(entries.lines().nth(2).unwrap().ends_with(",,"), "an entry without an origin has empty cells: {entries}");
    assert!(file(&d, "Dictionary-metadata.json").contains("\"Borrowed_From\""));
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

const BLOB: &str = "blake3:0000000000000000000000000000000000000000000000000000000000000001";

/// The first text linked to a recording whose audio is "on this computer".
fn with_recording(b: &mut Built) {
    let src = b.p.add_source("Counting session", None, "audio", None).unwrap();
    b.p.add_media_track(&src, BLOB, "audio", "mic", 0).unwrap();
    let text = b.p.proj.entities_of_kind("text").unwrap().into_iter().find(|t| b.p.proj.field_str(t, "title").unwrap().as_deref() == Some("Counting, part 1")).unwrap();
    b.p.set_text_source(&text, Some(&src)).unwrap();
}

fn run_with_audio(b: &Built, include_restricted: bool) -> Dataset {
    let has = |blob: &str| blob == BLOB;
    export(&b.p, &Options { title: "Kanu dictionary", generator: "wugs test", transcription_tiers: &b.tiers.0, translation_tiers: &b.tiers.1, include_restricted, audio: Some(&has) }).unwrap()
}

#[test]
fn timed_examples_get_a_sound_clip_from_their_recording() {
    let mut b = build();
    assert!(!run(&b, true).files.iter().any(|(n, _)| n == "media.csv"), "no media table unless asked");
    with_recording(&mut b);
    let d = run_with_audio(&b, true);
    // The linked text's example gets a clip; the other text has no recording.
    assert_eq!(d.clips.len(), 1, "{:?}", d.clips);
    assert_eq!(d.clips_missing, 1);
    let clip = &d.clips[0];
    assert_eq!((clip.blob.as_str(), clip.start_ms, clip.end_ms), (BLOB, 0.0, 1200.0));
    assert!(clip.file.starts_with("audio/audio-seg_") && clip.file.ends_with(".wav"), "{}", clip.file);
    let media = file(&d, "media.csv");
    assert!(media.starts_with("ID,Name,Description,Media_Type,Download_URL\n"), "{media}");
    assert!(media.contains("\"Counting, part 1 0.00–1.20 s\",audio/wav,audio/audio-seg_"), "{media}");
    let examples = file(&d, "examples.csv");
    assert!(examples.lines().next().unwrap().ends_with(",Media_ID"));
    let id = clip.file.trim_start_matches("audio/").trim_end_matches(".wav");
    assert!(examples.lines().any(|l| l.ends_with(&format!(",{id}"))), "{examples}");
    assert!(d.notes.iter().any(|n| n.contains("1 timed example has no sound clip")), "{:?}", d.notes);
    assert!(file(&d, "README.md").contains("1 sound clip"));
    // A restricted recording gives no clip unless restricted material was asked for.
    let src = b.p.proj.entities_of_kind("source").unwrap().pop().unwrap();
    b.p.set_access(&src, "restricted").unwrap();
    assert_eq!(run_with_audio(&b, false).clips.len(), 0);
    assert_eq!(run_with_audio(&b, true).clips.len(), 1);
    // Audio not on this computer: no clip, said in the notes.
    let none = |_: &str| false;
    let d = export(&b.p, &Options { title: "K", generator: "t", transcription_tiers: &b.tiers.0, translation_tiers: &b.tiers.1, include_restricted: true, audio: Some(&none) }).unwrap();
    assert_eq!((d.clips.len(), d.clips_missing), (0, 2));
    std::fs::remove_dir_all(&b.root).ok();
}

#[test]
fn the_reference_validator_accepts_the_dataset() {
    let mut b = build();
    with_recording(&mut b);
    let d = run_with_audio(&b, true);
    let dir = b.root.join("cldf-out");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, contents) in &d.files {
        std::fs::write(dir.join(name), contents).unwrap();
    }
    std::fs::create_dir_all(dir.join("audio")).unwrap();
    for c in &d.clips {
        std::fs::write(dir.join(&c.file), b"RIFF").unwrap();
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

#[test]
fn elicited_answers_export_as_a_cldf_wordlist_and_an_edictor_table() {
    let (root, mut p) = temp_project();
    p.set_project_language("Kanu", "abcd1234", "xyz").unwrap();
    let rows = [
        ConceptRow { key: "k1".into(), number: 1, gloss: "I".into(), concepticon: "1209".into(), concepticon_gloss: "I".into() },
        ConceptRow { key: "k2".into(), number: 2, gloss: "you (singular)".into(), concepticon: "1215".into(), concepticon_gloss: "THOU".into() },
        ConceptRow { key: "k3".into(), number: 3, gloss: "snow".into(), concepticon: "1240".into(), concepticon_gloss: "SNOW".into() },
    ];
    p.import_concept_list("test:swadesh", "Test Swadesh", "", &rows).unwrap();
    let (( _, _), _) = p.elicit(&concept_id("test:swadesh", "k1"), "nia", "", None).unwrap();
    let (( _, _), _) = p.elicit(&concept_id("test:swadesh", "k2"), "ngo, \"ka\"", "thou (familiar)", None).unwrap();
    p.set_concept_skipped(&concept_id("test:swadesh", "k3"), true, "no snow here").unwrap();
    let cut = |w: &str| -> Vec<String> { if w.starts_with("ng") { vec!["ŋ".into(), "o".into()] } else { w.chars().filter(|c| c.is_alphabetic()).map(|c| c.to_string()).collect() } };
    let wl = export_wordlist(&p, &WordlistOptions { title: "Kanu word list", generator: "wugs test", list: None, segment: &cut }).unwrap();
    assert_eq!((wl.lists, wl.concepts, wl.answered, wl.forms, wl.skipped), (1, 3, 2, 2, 1));
    let file = |name: &str| wl.files.iter().find(|(n, _)| n == name).map(|(_, c)| c.clone()).unwrap();
    let forms = file("forms.csv");
    assert!(forms.lines().nth(1).unwrap().contains(",abcd1234,") && forms.contains(",nia,nia,n i a,"), "{forms}");
    assert!(forms.contains("ŋ o,thou (familiar),thou (familiar)"), "{forms}");
    let edictor = file("edictor.tsv");
    assert_eq!(edictor.lines().next().unwrap(), "ID\tDOCULECT\tCONCEPT\tCONCEPTICON_ID\tVALUE\tFORM\tTOKENS\tNOTE");
    assert_eq!(edictor.lines().nth(1).unwrap(), "1\tKanu\tI\t1209\tnia\tnia\tn i a\t");
    assert!(file("parameters.csv").contains("SNOW"));
    let dir = root.join("wordlist");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, contents) in &wl.files {
        std::fs::write(dir.join(name), contents).unwrap();
    }
    match Command::new("cldf").arg("validate").arg(dir.join("Wordlist-metadata.json")).output() {
        Ok(out) => {
            let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            assert!(out.status.success() && !said.contains("WARNING") && !said.contains("ERROR"), "cldf validate:\n{said}");
        }
        Err(_) => eprintln!("pycldf is not installed (pip install pycldf); skipped the reference validator"),
    }
    assert!(export_wordlist(&p, &WordlistOptions { title: "x", generator: "x", list: Some("cpl_nope"), segment: &cut }).is_err());
    drop(p);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn typology_answers_export_as_a_cldf_structure_dataset() {
    let (root, mut p) = temp_project();
    p.set_project_language("Kanu", "abcd1234", "xyz").unwrap();
    let codes = |v: &[(&str, &'static str)]| v.iter().map(|(a, b)| (a.to_string(), *b)).collect::<Vec<_>>();
    let features = [
        structure::Feature { id: "GB020", question: "Are there definite or specific articles?", codes: codes(&[("0", "absent"), ("1", "present")]) },
        structure::Feature { id: "GB021", question: "Do indefinite nominals commonly have indefinite articles?", codes: codes(&[("0", "absent"), ("1", "present")]) },
        structure::Feature { id: "GB022", question: "Are there prenominal articles?", codes: codes(&[("0", "absent"), ("1", "present")]) },
    ];
    let answers = [
        structure::Answer { feature: "GB020", value: "1", note: "the word «ang», as in \"ang aso\"" },
        structure::Answer { feature: "GB021", value: "?", note: "" },
    ];
    let s = export_structure(&p, &StructureOptions { title: "Kanu typology", generator: "wugs test", source: "Grambank 1.0", features: &features, answers: &answers }).unwrap();
    assert_eq!((s.coded, s.unknown), (1, 1));
    let file = |name: &str| s.files.iter().find(|(n, _)| n == name).map(|(_, c)| c.clone()).unwrap();
    let values = file("values.csv");
    assert!(values.contains("abcd1234-GB020,abcd1234,GB020,1,GB020-1,"), "{values}");
    assert!(values.contains("abcd1234-GB021,abcd1234,GB021,?,,"), "an unknown has no code: {values}");
    assert!(!file("parameters.csv").contains("GB022"), "an unanswered feature is left out");
    let dir = root.join("structure");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, contents) in &s.files {
        std::fs::write(dir.join(name), contents).unwrap();
    }
    match Command::new("cldf").arg("validate").arg(dir.join("StructureDataset-metadata.json")).output() {
        Ok(out) => {
            let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            assert!(out.status.success() && !said.contains("WARNING") && !said.contains("ERROR"), "cldf validate:\n{said}");
        }
        Err(_) => eprintln!("pycldf is not installed (pip install pycldf); skipped the reference validator"),
    }
    let bad = [structure::Answer { feature: "GB020", value: "7", note: "" }];
    assert!(export_structure(&p, &StructureOptions { title: "x", generator: "x", source: "x", features: &features, answers: &bad }).is_err());
    drop(p);
    std::fs::remove_dir_all(&root).ok();
}
