use std::fs;
use std::path::PathBuf;

use wugs_desktop::gloss::{self, MorphSpecView};
use wugs_desktop::session::{self, Session};
use wugs_model::ulid;

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn created() -> (TempDir, Session) {
    let dir = TempDir(std::env::temp_dir().join(format!("wugs-desktop-gloss-{}", ulid())));
    let mut s: Session = None;
    session::create_project(&mut s, dir.0.to_str().unwrap(), "prs_Alice").unwrap();
    (dir, s)
}

fn segment(s: &mut Session, title: &str, content: &str) -> (String, String) {
    let text = session::create_text(s, title).unwrap();
    let seg = session::add_segment(s, &text, None, Some(0.0), Some(1000.0), content).unwrap().id;
    (text, seg)
}

fn m(surface: &str, ty: &str, gloss: Option<&str>) -> MorphSpecView {
    MorphSpecView { surface: surface.into(), morph_type: ty.into(), gloss: gloss.map(str::to_string), glosses_by_lang: Default::default(), sense: None }
}

fn forms(v: &[&str]) -> Vec<String> {
    v.iter().map(|f| f.to_string()).collect()
}

#[test]
fn glossing_gets_faster_as_the_project_learns_a_form() {
    let (_dir, mut s) = created();
    let (t1, g1) = segment(&mut s, "one", "köpekler geldi");
    let (t2, g2) = segment(&mut s, "two", "köpekler");

    let edit = gloss::gloss_tokenize(&mut s, &t1, &g1, &forms(&["köpekler", "geldi"])).unwrap();
    assert_eq!(edit.view.words.iter().map(|w| w.form.as_str()).collect::<Vec<_>>(), ["köpekler", "geldi"]);
    assert!(edit.view.words.iter().all(|w| w.morphs.is_empty() && w.suggestions.is_empty()), "nothing to suggest yet");
    let first = edit.view.words[0].id.clone();

    let dog = vec![m("köpek", "root", Some("dog")), m("ler", "suffix", Some("PL"))];
    let edit = gloss::gloss_apply(&mut s, &t1, &g1, &first, dog.clone()).unwrap();
    assert_eq!(edit.view.words[0].morphs.len(), 2);
    assert!(edit.view.words[0].findings.is_empty(), "{:?}", edit.view.words[0].findings);

    gloss::gloss_tokenize(&mut s, &t2, &g2, &forms(&["Köpekler"])).unwrap();
    let view = gloss::gloss_get(&mut s, &t2, &g2).unwrap();
    let word = &view.words[0];
    assert_eq!(word.suggestions.len(), 1, "an earlier analysis of the same form, case folded");
    assert_eq!((word.suggestions[0].count, word.suggestions[0].morphs.clone()), (1, dog.clone()));
    assert_eq!(gloss::gloss_get(&mut s, &t1, &g1).unwrap().words[0].others_unanalysed, 1, "the analysed word knows one other waits");

    let done = gloss::gloss_apply_all(&mut s, &t1, &g1, "köpekler", dog).unwrap();
    assert_eq!((done.applied, done.texts, done.skipped_analysed), (1, 1, 1));
    assert_eq!(done.view.words[0].others_unanalysed, 0);
    assert_eq!(gloss::gloss_get(&mut s, &t2, &g2).unwrap().words[0].morphs.len(), 2, "the other text's token was glossed");
}

#[test]
fn glosses_senses_and_the_lexicon_loop() {
    let (_dir, mut s) = created();
    let (t, g) = segment(&mut s, "one", "köpekler");
    let word = gloss::gloss_tokenize(&mut s, &t, &g, &forms(&["köpekler"])).unwrap().view.words[0].id.clone();
    let edit = gloss::gloss_apply(&mut s, &t, &g, &word, vec![m("köpek", "root", None), m("ler", "suffix", Some("PL"))]).unwrap();
    let root = edit.view.words[0].morphs[0].id.clone();
    assert!(edit.view.words[0].findings.iter().any(|f| f.starts_with("gloss-missing")), "the bare root is flagged, not blocked");

    assert!(gloss::gloss_lexicon(&mut s, "köpek").unwrap().is_empty());
    let edit = gloss::gloss_add_lexeme(&mut s, &t, &g, &root, "köpek", "dog").unwrap();
    let morph = &edit.view.words[0].morphs[0];
    assert_eq!(morph.sense_label.as_deref(), Some("köpek \u{2013} dog"));
    assert!(morph.spec.sense.is_some());
    let found = gloss::gloss_lexicon(&mut s, "KÖPEK").unwrap();
    assert_eq!(found.len(), 1);
    let again = gloss::gloss_add_lexeme(&mut s, &t, &g, &root, "köpek", "dog").unwrap();
    assert_eq!(again.view.words[0].morphs[0].spec.sense.as_deref(), Some(found[0].sense.as_str()), "a second add links the entry");
    assert_eq!(gloss::gloss_lexicon(&mut s, "köpek").unwrap().len(), 1, "no duplicate entry");

    let edit = gloss::gloss_pick_sense(&mut s, &t, &g, &root, &found[0].sense, &found[0].gloss).unwrap();
    assert_eq!(edit.view.words[0].morphs[0].spec.gloss.as_deref(), Some("dog"), "picking sets the gloss too");
    let edit = gloss::gloss_set(&mut s, &t, &g, &root, "perro", Some("es")).unwrap();
    assert_eq!(edit.view.words[0].morphs[0].spec.glosses_by_lang.get("es").map(String::as_str), Some("perro"));
    assert!(gloss::gloss_set(&mut s, &t, &g, &root, "x", Some("not a tag")).is_err());
}

#[test]
fn retokenize_reports_lost_analyses_and_keeps_the_rest() {
    let (_dir, mut s) = created();
    let (t, g) = segment(&mut s, "one", "köpekler geldi");
    let words = gloss::gloss_tokenize(&mut s, &t, &g, &forms(&["köpekler", "geldi"])).unwrap().view.words;
    gloss::gloss_apply(&mut s, &t, &g, &words[0].id, vec![m("köpek", "root", Some("dog")), m("ler", "suffix", Some("PL"))]).unwrap();
    gloss::gloss_apply(&mut s, &t, &g, &words[1].id, vec![m("geldi", "root", Some("come.PST"))]).unwrap();

    let edit = gloss::gloss_retokenize(&mut s, &t, &g, &forms(&["köpekler", "gitti"])).unwrap();
    assert_eq!(edit.view.words[0].morphs.len(), 2, "the unchanged word kept its analysis");
    assert!(edit.view.words[1].morphs.is_empty());
    assert!(edit.notes.iter().any(|n| n.contains("1 analysed word")), "{:?}", edit.notes);
    assert_eq!(edit.view.words[0].id, words[0].id, "the unchanged word is the same token");
    assert!(edit.view.hidden.is_empty(), "dropping your own analysis is not a hidden edit");
}

#[test]
fn a_segment_from_another_text_is_refused() {
    let (_dir, mut s) = created();
    let (t1, g1) = segment(&mut s, "one", "a");
    let (t2, _) = segment(&mut s, "two", "b");
    assert!(gloss::gloss_get(&mut s, &t2, &g1).is_err());
    assert!(gloss::gloss_tokenize(&mut s, &t2, &g1, &forms(&["a"])).is_err());
    assert!(gloss::gloss_get(&mut s, &t1, &g1).unwrap().words.is_empty());
}

#[test]
fn text_typed_decomposed_is_stored_composed() {
    let (_dir, mut s) = created();
    let decomposed = "c\u{327}ay ko\u{308}pek";
    let (t, g) = segment(&mut s, "one", decomposed);
    let edit = gloss::gloss_tokenize(&mut s, &t, &g, &forms(&["c\u{327}ay", "ko\u{308}pek"])).unwrap();
    let forms_stored: Vec<_> = edit.view.words.iter().map(|w| w.form.clone()).collect();
    assert_eq!(forms_stored, ["\u{e7}ay", "k\u{f6}pek"], "NFC at entry on the create path");
    let word = edit.view.words[1].id.clone();
    let edit = gloss::gloss_apply(&mut s, &t, &g, &word, vec![m("ko\u{308}pek", "root", Some("do\u{308}g"))]).unwrap();
    let morph = &edit.view.words[1].morphs[0];
    assert_eq!((morph.spec.surface.as_str(), morph.spec.gloss.as_deref()), ("k\u{f6}pek", Some("d\u{f6}g")), "and on morphs and glosses");
    let segs = session::get_text(&mut s, &t).unwrap().segments;
    assert_eq!(segs[0].content, "\u{e7}ay k\u{f6}pek", "segment content too");
}

#[test]
fn a_concurrent_analysis_is_offered_and_can_be_chosen() {
    let a_dir = TempDir(std::env::temp_dir().join(format!("wugs-desktop-gloss-a-{}", ulid())));
    let b_dir = TempDir(std::env::temp_dir().join(format!("wugs-desktop-gloss-b-{}", ulid())));
    let mut b = wugs_core::Project::open(&b_dir.0, "prs_Bob").unwrap();
    let text = b.create_text("T").unwrap();
    let seg = b.add_segment(&text, "tir_utt", 0, None, None, "köpekler").unwrap().0;
    let tok = b.tokenize(&text, &seg, &["köpekler"]).unwrap().0.remove(0);
    let receive = |dir: &TempDir, from: &wugs_core::Project| {
        let mut p = if dir.0.exists() { wugs_core::Project::open(&dir.0, "prs_Alice") } else { wugs_core::Project::open_joined(&dir.0, "prs_Alice", &from.project_id) }.unwrap();
        p.ingest(from.log.read_all().unwrap().ops).unwrap();
    };
    receive(&a_dir, &b);

    let mut s: Session = None;
    session::open_project(&mut s, a_dir.0.to_str().unwrap(), "prs_Alice").unwrap();
    gloss::gloss_apply(&mut s, &text, &seg, &tok, vec![m("köpek", "root", Some("dog")), m("ler", "suffix", Some("PL"))]).unwrap();
    b.apply_analysis(&text, &tok, &[wugs_core::MorphSpec { surface: "köpekler".into(), morph_type: "root".into(), gloss: Some("dogs".into()), ..Default::default() }]).unwrap();
    s = None;
    receive(&a_dir, &b);
    session::open_project(&mut s, a_dir.0.to_str().unwrap(), "prs_Alice").unwrap();

    let v = gloss::gloss_get(&mut s, &text, &seg).unwrap();
    assert_eq!(v.alternatives.len(), 1, "the other analysis is offered");
    let alt = &v.alternatives[0];
    assert_eq!(alt.parent, tok);
    let shown: Vec<&str> = v.words[0].morphs.iter().map(|m| m.spec.surface.as_str()).collect();
    let offered: Vec<&str> = alt.surfaces.iter().map(String::as_str).collect();
    assert!((shown == ["köpek", "ler"] && offered == ["köpekler"] && alt.glosses == ["dogs"]) || (shown == ["köpekler"] && offered == ["köpek", "ler"] && alt.glosses == ["dog", "PL"]), "{shown:?} vs {offered:?}");
    assert_eq!(session::conflict_count(&mut s).unwrap(), 1);

    let after = gloss::gloss_choose_list(&mut s, &text, &seg, &tok, &alt.members.clone()).unwrap().view;
    let now: Vec<&str> = after.words[0].morphs.iter().map(|m| m.spec.surface.as_str()).collect();
    assert_eq!(now, offered, "the chosen analysis shows");
    assert!(after.alternatives.is_empty(), "nothing left to choose");
    assert!(!after.hidden.is_empty(), "the analysis set aside stays restorable");
    assert_eq!(session::conflict_count(&mut s).unwrap(), 0, "the choice named both lists, so the conflict closed");
}
