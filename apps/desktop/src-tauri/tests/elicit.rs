use std::fs;
use std::path::PathBuf;

use wugs_desktop::session::{self, Session};
use wugs_desktop::{elicit, lexicon};
use wugs_model::ulid;

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn created() -> (TempDir, Session) {
    let dir = TempDir(std::env::temp_dir().join(format!("wugs-desktop-elicit-{}", ulid())));
    let mut s: Session = None;
    session::create_project(&mut s, dir.0.to_str().unwrap(), "prs_Alice").unwrap();
    (dir, s)
}

#[test]
fn a_bundled_list_loads_and_tracks_answers_and_skips() {
    let (_dir, mut s) = created();
    let lists = elicit::concept_lists_bundled(&mut s).unwrap();
    assert_eq!(lists.iter().map(|l| (l.source.as_str(), l.count)).collect::<Vec<_>>(), [
        ("concepticon:Swadesh-1955-100", 100),
        ("concepticon:Swadesh-1952-200", 200),
        ("concepticon:Tadmor-2009-100", 100),
    ]);
    assert!(lists.iter().all(|l| l.list_id.is_none()));
    let loaded = elicit::concept_list_load(&mut s, "concepticon:Tadmor-2009-100").unwrap();
    assert_eq!((loaded.total, loaded.answered), (100, 0));
    assert_eq!(elicit::concept_lists_bundled(&mut s).unwrap()[2].list_id.as_deref(), Some(loaded.id.as_str()));

    let view = elicit::concept_list_view(&mut s, &loaded.id).unwrap();
    assert_eq!((view.concepts[0].number, view.concepts[0].gloss.as_str()), (1, "fire"));
    let fire = view.concepts[0].id.clone();
    let nose = view.concepts[1].id.clone();
    let e = elicit::elicit_answer(&mut s, &fire, "ateş", "").unwrap();
    assert_eq!((e.concept.answers[0].lemma.as_str(), e.concept.answers[0].gloss.as_str()), ("ateş", "fire"));
    elicit::elicit_skip(&mut s, &nose, true, "only a loan").unwrap();
    let after = elicit::concept_list_view(&mut s, &loaded.id).unwrap().summary;
    assert_eq!((after.answered, after.skipped), (1, 1));

    // An existing entry can answer a second concept, and be taken back.
    let water = lexicon::lexicon_add(&mut s, "su", "water").unwrap().entry.unwrap();
    let w = elicit::concept_list_view(&mut s, &loaded.id).unwrap().concepts.into_iter().find(|c| c.gloss == "water").unwrap();
    let linked = elicit::elicit_link(&mut s, &w.id, &water.senses[0].id).unwrap();
    assert_eq!(linked.concept.answers.len(), 1);
    let unlinked = elicit::elicit_unlink(&mut s, &w.id, &water.senses[0].id).unwrap();
    assert!(unlinked.concept.answers.is_empty());

    // Deleting the answering entry uncovers the concept.
    lexicon::lexicon_delete(&mut s, &e.concept.answers[0].lexeme_id).unwrap();
    assert_eq!(elicit::concept_list_view(&mut s, &loaded.id).unwrap().summary.answered, 0);
}

#[test]
fn a_sil_domain_shows_its_questions_and_collects_words() {
    let (_dir, mut s) = created();
    lexicon::domains_load_sil(&mut s).unwrap();
    let moon = lexicon::domains_list(&mut s).unwrap().into_iter().find(|d| d.code == "1.1.1.1").unwrap();
    let q = elicit::domain_questions(&mut s, &moon.id).unwrap();
    assert!(q.questions.len() >= 10, "{}", q.questions.len());
    assert_eq!(q.questions[0].q, "What words refer to the moon?");
    assert!(q.words.is_empty());
    let e = elicit::domain_collect(&mut s, &moon.id, "ay", "moon").unwrap();
    assert_eq!(e.domain.words.iter().map(|w| w.lemma.as_str()).collect::<Vec<_>>(), ["ay"]);
    assert!(elicit::domain_collect(&mut s, &moon.id, " ", "").is_err());
}
