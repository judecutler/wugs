use std::fs;
use std::path::PathBuf;

use wugs_desktop::lexicon;
use wugs_desktop::session::{self, Session};
use wugs_model::ulid;

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn created() -> (TempDir, Session) {
    let dir = TempDir(std::env::temp_dir().join(format!("wugs-desktop-lexicon-{}", ulid())));
    let mut s: Session = None;
    session::create_project(&mut s, dir.0.to_str().unwrap(), "prs_Alice").unwrap();
    (dir, s)
}

#[test]
fn the_bundled_sil_list_parses_in_outline_order() {
    let rows = lexicon::sil_domain_rows().unwrap();
    assert_eq!(rows.len(), 1792);
    assert_eq!((rows[0].code.as_str(), rows[0].label.as_str()), ("1", "Universe, creation"));
    let mut seen = std::collections::HashSet::new();
    for r in &rows {
        if let Some((parent, _)) = r.code.rsplit_once('.') {
            assert!(seen.contains(parent), "{} before its parent", r.code);
        }
        seen.insert(r.code.as_str());
    }
}

#[test]
fn loading_sil_domains_builds_the_tree_once() {
    let (_dir, mut s) = created();
    let first = lexicon::domains_load_sil(&mut s).unwrap();
    assert_eq!((first.created, first.already), (1792, 0));
    assert!(first.notes.is_empty(), "{:?}", first.notes);
    let again = lexicon::domains_load_sil(&mut s).unwrap();
    assert_eq!((again.created, again.already), (0, 1792));
    let tree = lexicon::domains_list(&mut s).unwrap();
    assert_eq!(tree.len(), 1792);
    let codes: Vec<&str> = tree.iter().take(4).map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["1", "1.1", "1.1.1", "1.1.1.1"], "depth first, parents before children");
    assert_eq!(tree.iter().map(|d| d.depth).collect::<Vec<_>>()[..4], [0, 1, 2, 3]);
    let i = |code: &str| tree.iter().position(|d| d.code == code).unwrap();
    assert!(tree.iter().filter(|d| d.code.starts_with("1.")).all(|d| i(&d.code) < i("2")), "a subtree ends before the next top domain");
    assert!(i("2.1.1") < i("2.1.1.1") && i("2.1.1.1") < i("2.1.2"), "outline order");
}

#[test]
fn an_entry_grows_senses_glosses_and_domains() {
    let (_dir, mut s) = created();
    lexicon::domains_load_sil(&mut s).unwrap();
    let moon = lexicon::domains_list(&mut s).unwrap().into_iter().find(|d| d.code == "1.1.1.1").unwrap();
    assert_eq!(moon.label, "Moon");

    let e = lexicon::lexicon_add(&mut s, "ay", "moon").unwrap().entry.unwrap();
    let id = e.id.clone();
    let sense = e.senses[0].id.clone();
    lexicon::lexicon_set_field(&mut s, &id, "pos-of-speech", "n").unwrap();
    lexicon::lexicon_set_sense_gloss(&mut s, &sense, "luna", Some("es")).unwrap();
    lexicon::lexicon_set_sense_text(&mut s, &sense, "definition", "the moon in the sky").unwrap();
    let e = lexicon::lexicon_assign_domain(&mut s, &sense, &moon.id).unwrap().entry.unwrap();
    assert_eq!(e.pos_of_speech, "n");
    assert_eq!(e.senses[0].glosses_by_lang.get("es").map(String::as_str), Some("luna"));
    assert_eq!(e.senses[0].definition, "the moon in the sky");
    assert_eq!(e.senses[0].domains.iter().map(|d| d.code.as_str()).collect::<Vec<_>>(), ["1.1.1.1"]);

    let e = lexicon::lexicon_add_sense(&mut s, &id, "month").unwrap().entry.unwrap();
    assert_eq!(e.senses.iter().map(|x| x.gloss.as_str()).collect::<Vec<_>>(), ["moon", "month"]);
    let e = lexicon::lexicon_unassign_domain(&mut s, &sense, &moon.id).unwrap().entry.unwrap();
    assert!(e.senses[0].domains.is_empty());

    let list = lexicon::lexicon_list(&mut s, "MON").unwrap();
    assert_eq!(list.len(), 1, "a gloss matches, case folded");
    assert_eq!(list[0].glosses, ["moon", "month"]);
    assert!(lexicon::lexicon_list(&mut s, "zzz").unwrap().is_empty());
}

#[test]
fn used_in_counts_only_words_a_reader_can_see() {
    use wugs_desktop::gloss::{self, MorphSpecView};
    let (_dir, mut s) = created();
    let e = lexicon::lexicon_add(&mut s, "köpek", "dog").unwrap().entry.unwrap();
    let sense = e.senses[0].id.clone();
    let text = session::create_text(&mut s, "T").unwrap();
    let seg = session::add_segment(&mut s, &text, None, Some(0.0), Some(1000.0), "köpek").unwrap().id;
    let word = gloss::gloss_tokenize(&mut s, &text, &seg, &["köpek".to_string()]).unwrap().view.words[0].id.clone();
    let m = |sense: Option<String>| MorphSpecView { surface: "köpek".into(), morph_type: "root".into(), gloss: Some("dog".into()), glosses_by_lang: Default::default(), sense };
    gloss::gloss_apply(&mut s, &text, &seg, &word, vec![m(Some(sense.clone()))]).unwrap();
    assert_eq!(lexicon::lexicon_entry(&mut s, &e.id).unwrap().senses[0].occurrences, 1);
    // A re-analysis hides the linked morph: it no longer counts.
    let part = |surface: &str, ty: &str| MorphSpecView { surface: surface.into(), morph_type: ty.into(), gloss: Some("x".into()), glosses_by_lang: Default::default(), sense: None };
    gloss::gloss_apply(&mut s, &text, &seg, &word, vec![part("kö", "root"), part("pek", "suffix")]).unwrap();
    assert_eq!(lexicon::lexicon_entry(&mut s, &e.id).unwrap().senses[0].occurrences, 0, "the hidden morph does not count");
    gloss::gloss_apply(&mut s, &text, &seg, &word, vec![m(Some(sense.clone()))]).unwrap();
    assert_eq!(lexicon::lexicon_entry(&mut s, &e.id).unwrap().senses[0].occurrences, 1);
    session::delete_segment(&mut s, &text, &seg).unwrap();
    assert_eq!(lexicon::lexicon_entry(&mut s, &e.id).unwrap().senses[0].occurrences, 0, "a deleted segment's words do not count");
}

#[test]
fn a_second_entry_with_the_same_lemma_warns_and_shows_its_twin() {
    let (_dir, mut s) = created();
    let a = lexicon::lexicon_add(&mut s, "yüz", "face").unwrap().entry.unwrap();
    let b = lexicon::lexicon_add(&mut s, "yüz", "hundred").unwrap();
    assert!(b.notes.iter().any(|n| n.starts_with("duplicate-lemma")), "{:?}", b.notes);
    let b = b.entry.unwrap();
    assert_eq!(b.same_lemma, [(a.id.clone(), None)]);
    lexicon::lexicon_set_homograph(&mut s, &a.id, Some(1)).unwrap();
    let b = lexicon::lexicon_set_homograph(&mut s, &b.id, Some(2)).unwrap();
    assert!(b.notes.is_empty(), "{:?}", b.notes);
    let list = lexicon::lexicon_list(&mut s, "").unwrap();
    assert_eq!(list.iter().map(|r| r.homograph).collect::<Vec<_>>(), [Some(1), Some(2)]);
    let c = lexicon::lexicon_add(&mut s, "yüz", "to swim").unwrap().entry.unwrap();
    let c = lexicon::lexicon_number_homographs(&mut s, &c.id).unwrap();
    assert_eq!(c.entry.unwrap().homograph, Some(3));
    assert!(c.notes.is_empty(), "{:?}", c.notes);
    let gone = lexicon::lexicon_delete(&mut s, &a.id).unwrap();
    assert!(gone.entry.is_none());
    assert_eq!(lexicon::lexicon_list(&mut s, "").unwrap().len(), 2);
    assert!(lexicon::lexicon_entry(&mut s, &a.id).is_err());
}
