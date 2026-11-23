//! Import → export → diff (PRD-TONGUES §5, SCROLLS.md §2): a CoNLL-U file in
//! the order the format asks for comes back byte for byte, and edits made in
//! wugs show in what goes out.

use std::path::PathBuf;

use wugs_conllu::{export, format, import_conllu, ExportSentence};
use wugs_core::{MorphSpec, Project};
use wugs_model::Value;

const SAMPLE: &str = include_str!("fixtures/sample.conllu");

fn temp_project() -> (PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("wugs-conllu-test-{}", wugs_model::ulid()));
    let p = Project::open(&root, "prs_Tester").unwrap();
    (root, p)
}

fn tier(p: &mut Project) -> String {
    p.add_tier("sentences", "time-aligned", Some("transcription"), None).unwrap().0
}

/// A text's segments on `tier`, in order, as export input.
fn sentences(p: &Project, text: &str, tier: &str) -> Vec<ExportSentence> {
    p.proj
        .ordered_children("text", text, "segment")
        .unwrap()
        .into_iter()
        .filter(|s| p.proj.field_str(s, "tier").unwrap().as_deref() == Some(tier))
        .map(|segment| ExportSentence { segment, translations: Vec::new() })
        .collect()
}

fn export_all(p: &Project, texts: &[String], tier: &str) -> String {
    texts.iter().map(|t| export(p, "unused", &sentences(p, t, tier)).unwrap().conllu).collect()
}

#[test]
fn a_file_comes_back_byte_for_byte() {
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    let r = import_conllu(&mut p, SAMPLE, "sample", &tier).unwrap();
    assert_eq!(r.texts.len(), 2, "one text per # newdoc");
    assert_eq!((r.sentences, r.words, r.ranges, r.empty_nodes), (3, 14, 2, 1));
    assert_eq!(p.proj.field_str(&r.texts[0], "title").unwrap().as_deref(), Some("playa"));
    assert_eq!(export_all(&p, &r.texts, &tier), SAMPLE);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn the_round_trip_survives_a_replay_from_the_log() {
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    let texts = import_conllu(&mut p, SAMPLE, "sample", &tier).unwrap().texts;
    drop(p);
    std::fs::remove_dir_all(root.join("cache")).unwrap();
    let p = Project::open(&root, "prs_Tester").unwrap();
    assert_eq!(export_all(&p, &texts, &tier), SAMPLE);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn segments_read_as_the_sentence_and_punctuation_is_not_a_word() {
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    let texts = import_conllu(&mut p, SAMPLE, "sample", &tier).unwrap().texts;
    let segs = sentences(&p, &texts[0], &tier);
    assert_eq!(p.proj.field_str(&segs[0].segment, "content").unwrap().as_deref(), Some("Vámonos al mar."));
    // No `# text`: the surface, with SpaceAfter=No honoured.
    assert_eq!(p.proj.field_str(&segs[1].segment, "content").unwrap().as_deref(), Some("María nada y Juan."));
    let toks = p.proj.members(&segs[0].segment).unwrap();
    assert_eq!(toks.len(), 6, "syntactic words only");
    assert_eq!(p.proj.field_str(&toks[5], "kind").unwrap().as_deref(), Some("punct"));
    assert_eq!(p.proj.field_str(&toks[0], "kind").unwrap(), None);
    // One op for the sentence's annotation: a row per word plus the empty node.
    let (analysis, _) = p.proj.field(&segs[1].segment, "analysis").unwrap().unwrap();
    let Value::Arr(rows) = analysis else { panic!() };
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0].get("head").and_then(Value::as_str), p.proj.members(&segs[1].segment).unwrap().get(1).map(String::as_str), "HEAD names the head's token");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn the_same_file_again_adds_nothing() {
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    import_conllu(&mut p, SAMPLE, "sample", &tier).unwrap();
    let again = import_conllu(&mut p, SAMPLE, "sample", &tier).unwrap();
    assert!(again.texts.is_empty());
    assert_eq!(again.matched.len(), 2);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_broken_file_is_refused_whole() {
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    let broken = format!("{SAMPLE}1\tok\t_\t_\t_\t_\t0\troot\t_\t_\n3\tgap\t_\t_\t_\t_\t1\tdep\t_\t_\n");
    let err = import_conllu(&mut p, &broken, "sample", &tier).unwrap_err();
    assert!(err.contains("line 31"), "{err}");
    assert!(p.proj.entities_of_kind("text").unwrap().is_empty(), "nothing half-imported");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn edits_in_wugs_show_in_the_export_and_the_tree_keeps_its_words() {
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    let texts = import_conllu(&mut p, SAMPLE, "sample", &tier).unwrap().texts;
    let seg = sentences(&p, &texts[1], &tier)[0].segment.clone();
    // Retype the transcription and retokenize: matched words keep their ids,
    // so heads still point at them; `# text` follows the new transcription.
    p.edit_content(&texts[1], &seg, "La gran ciudad duerme").unwrap();
    p.retokenize(&texts[1], &seg, &["La", "gran", "ciudad", "duerme"]).unwrap();
    let out = export(&p, "ciudad", &sentences(&p, &texts[1], &tier)).unwrap();
    let parsed = format::parse(&out.conllu).unwrap();
    assert_eq!(parsed[0].comment("text"), Some("La gran ciudad duerme"));
    let words: Vec<(u32, &format::Cols)> = parsed[0].words().collect();
    assert_eq!(words.len(), 4);
    assert_eq!(words[1].1[format::FORM], "gran");
    assert_eq!(words[1].1[format::LEMMA], "_", "a new word has no analysis yet");
    assert_eq!(words[2].1[format::HEAD], "4", "ciudad still depends on duerme, now word 4");
    assert_eq!(words[0].1[format::HEAD], "3", "La still depends on ciudad, now word 3");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_glossed_wugs_text_exports_with_its_analysis_in_misc() {
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    let text = p.create_text("Counting").unwrap();
    let (seg, _) = p.add_segment(&text, &tier, 0, Some(0.0), Some(1000.0), "nikani tekama.").unwrap();
    let (toks, _) = p.tokenize(&text, &seg, &["nikani", "tekama"]).unwrap();
    let ((lex, sense), _) = p.add_lexeme("kani", "count").unwrap();
    p.set_lexeme_field(&lex, "pos-of-speech", "Verb").unwrap();
    let m = |surface: &str, kind: &str, gloss: &str, sense: Option<&str>| MorphSpec { surface: surface.into(), morph_type: kind.into(), gloss: Some(gloss.into()), glosses_by_lang: Default::default(), sense: sense.map(str::to_string) };
    p.apply_analysis(&text, &toks[0], &[m("ni-", "prefix", "1SG", None), m("kani", "root", "count", Some(&sense))]).unwrap();
    p.apply_analysis(&text, &toks[1], &[m("teka", "root", "ten", None)]).unwrap();
    let out = export(&p, "Counting", &[ExportSentence { segment: seg, translations: vec![(Some("en".into()), "I count to ten.".into()), (None, "Cuento hasta diez.".into())] }]).unwrap();
    let expected = "# newdoc id = Counting\n# sent_id = 1\n# text = nikani tekama.\n# text_en = I count to ten.\n# translation = Cuento hasta diez.\n\
1\tnikani\tkani\tVERB\tVerb\t_\t_\t_\t_\tMSeg=ni-kani|MGloss=1SG-count\n\
2\ttekama\t_\t_\t_\t_\t_\t_\t_\tGloss=ten\n\n";
    assert_eq!(out.conllu, expected);
    assert!(format::parse(&out.conllu).is_ok());
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn an_untokenized_segment_is_split_at_spaces_and_says_so() {
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    let text = p.create_text("Notes").unwrap();
    let (seg, _) = p.add_segment(&text, &tier, 0, None, None, "two words").unwrap();
    let (empty, _) = p.add_segment(&text, &tier, 1, None, None, "").unwrap();
    let out = export(&p, "Notes", &[ExportSentence { segment: seg, translations: Vec::new() }, ExportSentence { segment: empty, translations: Vec::new() }]).unwrap();
    assert_eq!(format::parse(&out.conllu).unwrap()[0].words().count(), 2);
    assert_eq!(out.notes.len(), 2, "{:?}", out.notes);
    std::fs::remove_dir_all(root).ok();
}
