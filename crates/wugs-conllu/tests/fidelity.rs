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
    let first = import_conllu(&mut p, SAMPLE, "sample", &tier).unwrap();
    let meta = p.text_metadata(&first.texts[0]).unwrap();
    assert_eq!(meta, vec![("author".to_string(), "Invented Author".to_string()), ("genre".to_string(), "conversation".to_string())], "GUM's # meta:: lines");
    assert!(p.text_metadata(&first.texts[1]).unwrap().is_empty(), "metadata stays with its document");
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
    assert!(err.contains("line 33"), "{err}");
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
    p.set_text_metadata(&text, "Speaker", "A").unwrap();
    let out = export(&p, "Counting", &[ExportSentence { segment: seg, translations: vec![(Some("en".into()), "I count to ten.".into()), (None, "Cuento hasta diez.".into())] }]).unwrap();
    let expected = "# newdoc id = Counting\n# meta::speaker = A\n# sent_id = 1\n# text = nikani tekama.\n# text_en = I count to ten.\n# translation = Cuento hasta diez.\n\
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

#[test]
fn vertical_and_tagged_text_carry_the_tags() {
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    let texts = import_conllu(&mut p, SAMPLE, "sample", &tier).unwrap().texts;
    let mut s = sentences(&p, &texts[0], &tier);
    s[0].translations.push((Some("en".into()), "Let's go to \"the\" sea.".into()));
    let v = wugs_conllu::vertical(&p, "playa & co", &texts[0], &s).unwrap().conllu;
    assert!(v.starts_with(&format!("<text id=\"{}\" title=\"playa &amp; co\" author=\"Invented Author\" genre=\"conversation\">\n<s id=\"1\" translation=\"Let&apos;s go to &quot;the&quot; sea.\">\nVamos\tir\tVERB\tV\tMood=Imp|Number=Plur|Person=1\n", texts[0])), "{v}");
    assert!(v.contains("mar\tmar\tNOUN\tNC\tNumber=Sing\n<g/>\n.\t.\tPUNCT\tFP\t_\n</s>"), "glue where no space follows: {v}");
    assert!(v.trim_end().ends_with("</text>"));
    let t = wugs_conllu::tagged_text(&p, "playa", &s).unwrap().conllu;
    assert_eq!(t.lines().next(), Some("Vamos_VERB nos_PRON a_ADP el_DET mar_NOUN ._PUNCT"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_correction_is_one_write_and_shows_in_the_export() {
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    let texts = import_conllu(&mut p, SAMPLE, "sample", &tier).unwrap().texts;
    let seg = sentences(&p, &texts[1], &tier)[0].segment.clone();
    let before = wugs_conllu::tree::rows(&p, &seg).unwrap();
    assert!(before.annotated && before.problems.is_empty());
    assert_eq!(before.rows[0].head, Some(2), "La depends on ciudad");
    let la = before.rows[0].token.clone();
    // La's head moved to the verb, its UPOS retyped in lower case.
    wugs_conllu::tree::set_cell(&mut p, &texts[1], &seg, &la, "head", "3").unwrap();
    wugs_conllu::tree::set_cell(&mut p, &texts[1], &seg, &la, "upos", "pron").unwrap();
    let out = export(&p, "ciudad", &sentences(&p, &texts[1], &tier)).unwrap().conllu;
    let w: Vec<format::Cols> = format::parse(&out).unwrap()[0].words().map(|(_, c)| c.clone()).collect();
    assert_eq!((w[0][format::HEAD].as_str(), w[0][format::UPOS].as_str()), ("3", "PRON"));
    // Bad values are refused before anything is written.
    assert!(wugs_conllu::tree::set_cell(&mut p, &texts[1], &seg, &la, "upos", "NOUNY").unwrap_err().contains("not a UD"));
    assert!(wugs_conllu::tree::set_cell(&mut p, &texts[1], &seg, &la, "feats", "Plural").unwrap_err().contains("Name=Value"));
    assert!(wugs_conllu::tree::set_cell(&mut p, &texts[1], &seg, &la, "head", "1").unwrap_err().contains("own head"));
    assert!(wugs_conllu::tree::set_cell(&mut p, &texts[1], &seg, &la, "head", "9").unwrap_err().contains("no word 9"));
    // A second root is reported, not refused.
    wugs_conllu::tree::set_cell(&mut p, &texts[1], &seg, &la, "head", "0").unwrap();
    assert!(wugs_conllu::tree::rows(&p, &seg).unwrap().problems.iter().any(|m| m.contains("2 words are the root")));
    // The sentence with an empty node keeps it through a correction.
    let seg2 = sentences(&p, &texts[0], &tier)[1].segment.clone();
    let maria = wugs_conllu::tree::rows(&p, &seg2).unwrap().rows[0].token.clone();
    wugs_conllu::tree::set_cell(&mut p, &texts[0], &seg2, &maria, "lemma", "María José").unwrap();
    let back = export(&p, "playa", &sentences(&p, &texts[0], &tier)).unwrap().conllu;
    assert!(back.contains("2.1\tnada\tnadar"), "{back}");
    assert!(back.contains("1\tMaría\tMaría José\tPROPN"), "{back}");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_sentence_with_no_analysis_gets_one_at_its_first_correction() {
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    let text = p.create_text("Notes").unwrap();
    let (seg, _) = p.add_segment(&text, &tier, 0, None, None, "dogs bark").unwrap();
    let (toks, _) = p.tokenize(&text, &seg, &["dogs", "bark"]).unwrap();
    assert!(!wugs_conllu::tree::rows(&p, &seg).unwrap().annotated);
    wugs_conllu::tree::set_cell(&mut p, &text, &seg, &toks[1], "head", "0").unwrap();
    wugs_conllu::tree::set_cell(&mut p, &text, &seg, &toks[0], "head", "2").unwrap();
    wugs_conllu::tree::set_cell(&mut p, &text, &seg, &toks[0], "deprel", "nsubj").unwrap();
    let r = wugs_conllu::tree::rows(&p, &seg).unwrap();
    assert!(r.annotated && r.problems.is_empty(), "{:?}", r.problems);
    assert_eq!((r.rows[0].head, r.rows[0].deprel.as_str()), (Some(2), "nsubj"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_taggers_output_annotates_segments_without_touching_glosses() {
    use wugs_conllu::annotate::{annotate, F_TAGGED_BY};
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    let text = p.create_text("Typed").unwrap();
    let (bare, _) = p.add_segment(&text, &tier, 0, None, None, "The dog runs.").unwrap();
    let (glossed, _) = p.add_segment(&text, &tier, 1, None, None, "Dogs run.").unwrap();
    let (toks, _) = p.tokenize(&text, &glossed, &["Dogs", "run"]).unwrap();
    let m = |surface: &str, kind: &str, gloss: &str| MorphSpec { surface: surface.into(), morph_type: kind.into(), gloss: Some(gloss.into()), glosses_by_lang: Default::default(), sense: None };
    p.apply_analysis(&text, &toks[0], &[m("dog", "root", "dog"), m("-s", "suffix", "PL")]).unwrap();
    let (other, _) = p.add_segment(&text, &tier, 2, None, None, "Cats sleep.").unwrap();
    p.tokenize(&text, &other, &["Cat", "s", "sleep"]).unwrap();
    let tagged = "1\tThe\tthe\tDET\tDT\t_\t2\tdet\t_\t_\n2\tdog\tdog\tNOUN\tNN\tNumber=Sing\t3\tnsubj\t_\t_\n3\truns\trun\tVERB\tVBZ\t_\t0\troot\t_\tSpaceAfter=No\n4\t.\t.\tPUNCT\t.\t_\t3\tpunct\t_\t_\n\n\
1\tDogs\tdog\tNOUN\tNNS\tNumber=Plur\t2\tnsubj\t_\t_\n2\trun\trun\tVERB\tVBP\t_\t0\troot\t_\tSpaceAfter=No\n3\t.\t.\tPUNCT\t.\t_\t2\tpunct\t_\t_\n\n\
1\tCats\tcat\tNOUN\tNNS\t_\t2\tnsubj\t_\t_\n2\tsleep\tsleep\tVERB\tVBP\t_\t0\troot\t_\tSpaceAfter=No\n3\t.\t.\tPUNCT\t.\t_\t2\tpunct\t_\t_\n\n";
    let sentences = format::parse(tagged).unwrap();
    let items: Vec<(String, format::Sentence)> = vec![bare.clone(), glossed.clone(), other.clone()].into_iter().zip(sentences).collect();
    let r = annotate(&mut p, &text, &items, "udpipe test-model").unwrap();
    assert_eq!((r.annotated, r.tokenized, r.differ, r.kept), (2, 1, 1, 0), "{r:?}");
    // The bare segment got the tagger's tokens, punctuation marked as such.
    let bare_toks = p.proj.members(&bare).unwrap();
    assert_eq!(bare_toks.len(), 4);
    assert!(p.proj.is_punct_token(&bare_toks[3]));
    let rows = wugs_conllu::tree::rows(&p, &bare).unwrap();
    assert!(rows.annotated && rows.problems.is_empty(), "{:?}", rows.problems);
    assert_eq!((rows.rows[1].upos.as_str(), rows.rows[1].head), ("NOUN", Some(3)));
    assert_eq!(p.proj.field_str(&bare, F_TAGGED_BY).unwrap().as_deref(), Some("udpipe test-model"));
    // The glossed segment kept its tokens and morphs; its rows name them.
    assert_eq!(p.proj.members(&glossed).unwrap(), toks);
    assert_eq!(p.proj.members(&toks[0]).unwrap().len(), 2, "the analysis stays");
    let g = wugs_conllu::tree::rows(&p, &glossed).unwrap();
    assert_eq!((g.rows[0].lemma.as_str(), g.rows[0].upos.as_str(), g.rows[1].head), ("dog", "NOUN", Some(0)));
    // Split differently: left alone.
    assert!(p.proj.field(&other, wugs_conllu::F_ANALYSIS).unwrap().is_none());
    // Again: every annotated segment is kept as it is now.
    let again = annotate(&mut p, &text, &items, "udpipe test-model").unwrap();
    assert_eq!((again.annotated, again.kept), (0, 2));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn machine_tags_stay_marked_until_a_person_checks_or_corrects_them() {
    use wugs_conllu::annotate::{annotate, check_status, mark_checked, unchecked};
    let (root, mut p) = temp_project();
    let tier = tier(&mut p);
    let text = p.create_text("Typed").unwrap();
    let (a, _) = p.add_segment(&text, &tier, 0, None, None, "Dogs bark.").unwrap();
    let (b, _) = p.add_segment(&text, &tier, 1, None, None, "Cats sleep.").unwrap();
    let (c, _) = p.add_segment(&text, &tier, 2, None, None, "Birds sing.").unwrap();
    let tagged = "1\tDogs\tdog\tNOUN\t_\t_\t2\tnsubj\t_\t_\n2\tbark\tbark\tVERB\t_\t_\t0\troot\t_\t_\n\n\
1\tCats\tcat\tNOUN\t_\t_\t2\tnsubj\t_\t_\n2\tsleep\tsleep\tVERB\t_\t_\t0\troot\t_\t_\n\n\
1\tBirds\tbird\tNOUN\t_\t_\t2\tnsubj\t_\t_\n2\tsing\tsing\tVERB\t_\t_\t0\troot\t_\t_\n\n";
    let items: Vec<(String, format::Sentence)> = vec![a.clone(), b.clone(), c.clone()].into_iter().zip(format::parse(tagged).unwrap()).collect();
    annotate(&mut p, &text, &items, "udpipe english-ewt").unwrap();
    let all = vec![a.clone(), b.clone(), c.clone()];
    assert_eq!(unchecked(&p, &a).unwrap().as_deref(), Some("udpipe english-ewt"));
    assert_eq!(check_status(&p, &all).unwrap().unchecked, all);
    // A correction is a person's look at the sentence.
    let toks = p.proj.members(&b).unwrap();
    wugs_conllu::tree::set_cell(&mut p, &text, &b, &toks[0], "lemma", "kitty").unwrap();
    assert_eq!(unchecked(&p, &b).unwrap(), None);
    // Mark checked changes nothing in the analysis and clears the mark.
    let before = p.proj.field(&a, wugs_conllu::F_ANALYSIS).unwrap().unwrap().0;
    assert_eq!(mark_checked(&mut p, &text, &all).unwrap(), 2, "b was checked by its correction");
    assert_eq!(p.proj.field(&a, wugs_conllu::F_ANALYSIS).unwrap().unwrap().0, before);
    let st = check_status(&p, &all).unwrap();
    assert_eq!((st.analysed, st.unchecked.len()), (3, 0));
    assert_eq!(mark_checked(&mut p, &text, &all).unwrap(), 0, "nothing left writes nothing");
    // The marks survive a rebuild of the cache from the log.
    drop(p);
    std::fs::remove_dir_all(root.join("cache")).unwrap();
    let p = Project::open(&root, "prs_Tester").unwrap();
    assert_eq!(check_status(&p, &all).unwrap().unchecked.len(), 0);
    std::fs::remove_dir_all(root).ok();
}
