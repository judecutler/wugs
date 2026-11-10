//! FTS5 search over the projection (DATA-MODEL §8), driven through the
//! facade so the index is maintained by real intents.

use std::fs;

use wugs_core::Project;
use wugs_model::Value;

fn temp() -> (std::path::PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("wugs-fts-test-{}", wugs_model::ulid()));
    let p = Project::open(&root, "prs_T").unwrap();
    (root, p)
}

#[test]
fn search_finds_lemmas_glosses_and_content_with_folding_and_prefix() {
    let (root, mut p) = temp();
    p.create_lexeme("köpek", "dog").unwrap();
    p.create_lexeme("kedi", "cat").unwrap();
    let txt = p.create_text("Frog story").unwrap();
    p.add_segment(&txt, "tir_utt", 0, None, None, "köpekler dün geldi").unwrap();

    let hits = p.proj.search("kopek", &[], 20).unwrap();
    assert_eq!(hits.len(), 2, "lemma and segment content both match without diacritics: {hits:?}");
    assert!(hits.iter().any(|h| h.kind == "lexeme" && h.field == "lemma"), "{hits:?}");
    assert!(hits.iter().any(|h| h.kind == "segment" && h.snippet.contains("[köpekler]")), "{hits:?}");

    let hits = p.proj.search("dog", &["sense"], 20).unwrap();
    assert_eq!(hits.len(), 1, "kind filter: {hits:?}");

    let hits = p.proj.search("kopek", &["lexeme", "sense"], 20).unwrap();
    assert!(!hits.is_empty() && hits.iter().all(|h| h.kind == "lexeme" || h.kind == "sense"), "several kinds: {hits:?}");

    let hits = p.proj.search("gel", &[], 20).unwrap();
    assert_eq!(hits.len(), 1, "prefix matches geldi: {hits:?}");
    fs::remove_dir_all(root).ok();
}

#[test]
fn edits_tombstones_and_replay_keep_the_index_true() {
    let (root, mut p) = temp();
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "eski metin").unwrap().0;

    let mut t = p.intent("retype");
    t.set("segment", &seg, &txt, "content", Value::str("yeni metin")).unwrap();
    t.commit().unwrap();
    assert!(p.proj.search("eski", &[], 10).unwrap().is_empty(), "old text gone from the index");
    assert_eq!(p.proj.search("yeni", &[], 10).unwrap().len(), 1, "new text found");

    let mut t = p.intent("delete-segment");
    t.delete("segment", &seg, &txt);
    t.commit().unwrap();
    assert!(p.proj.search("yeni", &[], 10).unwrap().is_empty(), "tombstoned entity filtered");

    drop(p);
    fs::remove_dir_all(root.join("cache")).unwrap();
    let p2 = Project::open(&root, "prs_T").unwrap();
    assert!(p2.proj.search("yeni", &[], 10).unwrap().is_empty(), "rebuild agrees: still tombstoned");
    assert!(p2.proj.search("eski", &[], 10).unwrap().is_empty(), "rebuild agrees: old text gone");
    fs::remove_dir_all(root).ok();
}

#[test]
fn collaborative_edits_reach_the_index() {
    let (root, mut p) = temp();
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "ilk hali").unwrap().0;
    p.edit_content(&txt, &seg, "köpek havladı").unwrap();
    assert!(p.proj.search("ilk", &[], 10).unwrap().is_empty(), "replaced text out of the index");
    assert_eq!(p.proj.search("havla", &[], 10).unwrap().len(), 1, "text-edit content indexed");
    fs::remove_dir_all(root).ok();
}

#[test]
fn hostile_queries_are_data_not_syntax() {
    let (root, mut p) = temp();
    let txt = p.create_text("T").unwrap();
    p.add_segment(&txt, "tir_utt", 0, None, None, "NEAR AND OR NOT \"quoted\"").unwrap();
    for q in ["NEAR(", "\"unclosed", "a AND", "col:x", "*", "(((", "near and"] {
        let r = p.proj.search(q, &[], 10);
        assert!(r.is_ok(), "query {q:?} must not error: {r:?}");
    }
    assert_eq!(p.proj.search("quoted", &[], 10).unwrap().len(), 1, "quoted word is findable as data");
    assert!(p.proj.search("", &[], 10).unwrap().is_empty(), "empty query, empty result");
    fs::remove_dir_all(root).ok();
}
