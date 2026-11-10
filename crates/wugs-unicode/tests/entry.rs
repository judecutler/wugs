//! NFC at entry, end to end through the facade.

use std::fs;
use std::path::PathBuf;

use wugs_core::{Engines, Project, SCOPE_CONFIG, SCOPE_LEXICON};
use wugs_model::Value;
use wugs_unicode::NfcNormalizer;

const NFD_CAY: &str = "c\u{0327}ay"; // typed with a combining cedilla
const NFC_CAY: &str = "\u{E7}ay";

fn temp_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("wugs-{tag}-{}", wugs_model::ulid()))
}

fn nfc() -> Engines {
    Engines { normalizer: Some(Box::new(NfcNormalizer)), ..Default::default() }
}

fn nfc_project(root: &PathBuf) -> Project {
    Project::open_with(root, "prs_T", nfc()).unwrap()
}

/// The value of `field` as the log holds it: from the create op's field
/// map, or from a set op on that field.
fn logged(p: &Project, target: &str, field: &str) -> String {
    let ops = p.log.read_all().unwrap().ops;
    let op = ops
        .iter()
        .rev()
        .find(|o| o.target == target && (o.verb == "create" || o.field.as_deref() == Some(field)))
        .unwrap_or_else(|| panic!("no op writes {target}.{field}"));
    let v = if op.verb == "create" { op.value.get(field).cloned() } else { Some(op.value.clone()) };
    v.and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| panic!("{target}.{field} not a string"))
}

#[test]
fn vernacular_text_is_stored_composed() {
    let root = temp_root("nfc-test");
    let mut p = nfc_project(&root);

    let (lex, _) = p.create_lexeme(NFD_CAY, "tea").unwrap();
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "c\u{0327}ay ic\u{0327}ti").unwrap().0;

    assert_eq!(p.proj.field_str(&seg, "content").unwrap().unwrap(), "\u{E7}ay i\u{E7}ti");
    assert_eq!(logged(&p, &seg, "content"), "\u{E7}ay i\u{E7}ti", "the log holds NFC content");
    assert_eq!(logged(&p, &lex, "lemma"), NFC_CAY, "the log holds an NFC lemma");
    assert!(!p.proj.search(NFC_CAY, &[], 10).unwrap().is_empty(), "precomposed query finds it");

    // collaborative edits normalize too
    p.edit_content(&txt, &seg, "c\u{0327}ok c\u{0327}ay").unwrap();
    assert_eq!(
        p.proj.field_str(&seg, "content").unwrap().unwrap(),
        "\u{E7}ok \u{E7}ay",
        "text-edit path normalizes before the CRDT diff"
    );

    let log = p.log.read_all().unwrap().ops;
    assert!(
        log.iter().all(|o| !o.to_json().contains('\u{0327}')),
        "no combining cedilla reaches the log"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn text_titles_are_stored_composed() {
    let root = temp_root("nfc-title");
    let mut p = nfc_project(&root);
    let txt = p.create_text(NFD_CAY).unwrap();
    assert_eq!(logged(&p, &txt, "title"), NFC_CAY, "create_scope_root normalizes like create");
    fs::remove_dir_all(root).ok();
}

#[test]
fn writing_system_and_audit_close_the_loop() {
    let root = temp_root("ws-test");
    let mut p = nfc_project(&root);

    // the charset and sort are typed decomposed too
    let charset = "abc\u{0327}defgg\u{0306}hıijklmnoo\u{0308}prss\u{0327}tuu\u{0308}vyz";
    let ws = p.create_writing_system("Proposed Turkish", "Latn", charset, "a b c c\u{0327} d e", "ltr").unwrap();
    assert!(!logged(&p, &ws, "charset").contains('\u{0327}'), "charset is stored in NFC");
    assert_eq!(logged(&p, &ws, "sort"), "a b c \u{E7} d e", "sort is stored in NFC");

    let mut t = p.intent("create-lexeme");
    t.create("lexeme", "lex", SCOPE_LEXICON, vec![("lemma", Value::str(NFD_CAY)), ("ws", Value::str(&ws))]);
    t.commit().unwrap();
    assert!(p.audit_writing_system(&ws).unwrap().is_empty(), "NFC entry and NFC charset agree");

    let mut t = p.intent("create-lexeme");
    let bad = t.create("lexeme", "lex", SCOPE_LEXICON, vec![("lemma", Value::str("queen")), ("ws", Value::str(&ws))]);
    t.commit().unwrap();
    let findings = p.audit_writing_system(&ws).unwrap();
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].0, bad);
    assert!(findings[0].2.contains('q'), "the offending characters are named");
    fs::remove_dir_all(root).ok();
}

#[test]
fn audit_follows_the_writing_system_down_from_the_text() {
    let root = temp_root("ws-inherit");
    let mut p = nfc_project(&root);
    let ws = p.create_writing_system("T", "Latn", "abcçd", "a b c ç d", "ltr").unwrap();
    let txt = p.create_text("T").unwrap();
    let mut t = p.intent("set-ws");
    t.set("text", &txt, &txt, "ws", Value::str(&ws)).unwrap();
    t.commit().unwrap();

    let bad_seg = p.add_segment(&txt, "tir_utt", 0, None, None, "abx").unwrap().0;
    let punct_seg = p.add_segment(&txt, "tir_utt", 1, None, None, "«ab, çd!» (1-2) … 'a'?").unwrap().0;
    let toks = p.tokenize(&txt, &punct_seg, &["ab", "qd"]).unwrap().0;
    let morphs = p.split_morph(&txt, &toks[0], &[("a", "root"), ("bz", "suffix")]).unwrap().0;

    let mut found: Vec<(String, String, String)> = p.audit_writing_system(&ws).unwrap();
    found.sort();
    let mut want = vec![
        (bad_seg, "content".to_string(), "x".to_string()),
        (toks[1].clone(), "form".to_string(), "q".to_string()),
        (morphs[1].clone(), "surface".to_string(), "z".to_string()),
    ];
    want.sort();
    assert_eq!(found, want, "segment, token and morph inherit; listed punctuation is exempt");
    fs::remove_dir_all(root).ok();
}

#[test]
fn audit_refuses_a_missing_or_deleted_writing_system() {
    let root = temp_root("ws-missing");
    let mut p = nfc_project(&root);
    assert!(p.audit_writing_system("ws_01ARZ3NDEKTSV4RRFFQ69G5FAV").is_err(), "unknown id");
    let ws = p.create_writing_system("T", "Latn", "ab", "a b", "ltr").unwrap();
    let mut t = p.intent("delete-ws");
    t.delete("ws", &ws, SCOPE_CONFIG);
    t.commit().unwrap();
    assert!(p.audit_writing_system(&ws).is_err(), "tombstoned id");
    fs::remove_dir_all(root).ok();
}

#[test]
fn ingest_and_replay_keep_text_as_written() {
    let (root_a, root_b) = (temp_root("nfd-a"), temp_root("nfd-b"));
    let mut a = Project::open(&root_a, "prs_A").unwrap(); // no normalizer
    let (lex, _) = a.create_lexeme(NFD_CAY, "tea").unwrap();
    let ops = a.log.read_all().unwrap().ops;

    let mut b = Project::open_joined_with(&root_b, "prs_B", &a.project_id, nfc()).unwrap();
    b.ingest(ops).unwrap();
    assert_eq!(logged(&b, &lex, "lemma"), NFD_CAY, "ingest logs the peer's bytes");
    assert_eq!(b.proj.field_str(&lex, "lemma").unwrap().unwrap(), NFD_CAY, "ingest projects them");

    drop(b);
    fs::remove_file(root_b.join("cache").join("projection.sqlite")).unwrap();
    let mut b = Project::open_with(&root_b, "prs_B", nfc()).unwrap();
    b.replay().unwrap();
    assert_eq!(b.proj.field_str(&lex, "lemma").unwrap().unwrap(), NFD_CAY, "replay applies the log verbatim");
    assert_eq!(logged(&b, &lex, "lemma"), NFD_CAY);
    fs::remove_dir_all(root_a).ok();
    fs::remove_dir_all(root_b).ok();
}

#[test]
fn suggestions_and_lookup_match_across_normalization_forms() {
    let root = temp_root("suggest-nfc");
    let mut p = nfc_project(&root);
    let (_, sense) = p.create_lexeme("köpek", "dog").unwrap();
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "x").unwrap().0;
    let decomposed = "ko\u{308}pekler";
    let composed = "k\u{f6}pekler";
    let toks = p.tokenize(&txt, &seg, &[decomposed, composed]).unwrap().0;
    p.apply_analysis(
        &txt,
        &toks[0],
        &[wugs_core::MorphSpec { surface: "ko\u{308}pek".into(), morph_type: "root".into(), gloss: Some("dog".into()), ..Default::default() }],
    )
    .unwrap();

    let got = p.analysis_suggestions(composed).unwrap();
    assert_eq!(got.len(), 1, "a decomposed token and a composed query are one form");
    assert_eq!(p.analysis_suggestions(decomposed).unwrap(), got);
    assert_eq!(p.unanalysed_with_form("K\u{d6}PEKLER").unwrap().len(), 1, "the composed token is the unanalysed one");
    let hits = p.lexicon_lookup("ko\u{308}pek").unwrap();
    assert_eq!((hits.len(), hits[0].sense.as_str()), (1, sense.as_str()), "a decomposed surface finds the composed lemma");
    fs::remove_dir_all(root).ok();
}

#[test]
fn morph_concatenation_compares_after_nfc() {
    let root = temp_root("concat-nfc");
    let mut p = Project::open(&root, "prs_T").unwrap(); // stores text as typed
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "x").unwrap().0;
    let tok = p.tokenize(&txt, &seg, &["ko\u{308}pekler"]).unwrap().0.remove(0);
    let (_, res) = p.split_morph(&txt, &tok, &[("k\u{f6}pek", "root"), ("ler", "suffix")]).unwrap();
    assert!(res.findings.iter().any(|f| f.code == "morph-concat"), "without NFC the two spellings differ");
    drop(p);

    let p = Project::open_with(&root, "prs_T", nfc()).unwrap();
    let found = p.token_findings(&[tok]).unwrap();
    assert!(found.iter().all(|f| f.code != "morph-concat"), "NFD form, NFC morphs: the same word: {found:?}");
    fs::remove_dir_all(root).ok();
}

#[test]
fn opening_with_nfc_rebuilds_keys_for_text_stored_as_typed() {
    let root = temp_root("keys-nfc");
    let mut p = Project::open(&root, "prs_T").unwrap();
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "x").unwrap().0;
    let tok = p.tokenize(&txt, &seg, &["ko\u{308}pekler"]).unwrap().0.remove(0);
    p.apply_analysis(&txt, &tok, &[wugs_core::MorphSpec { surface: "köpekler".into(), morph_type: "root".into(), gloss: Some("dogs".into()), ..Default::default() }]).unwrap();
    assert!(p.analysis_suggestions("k\u{f6}pekler").unwrap().is_empty(), "no normalizer: the spellings differ");
    drop(p);

    let p = Project::open_with(&root, "prs_T", nfc()).unwrap();
    assert_eq!(p.analysis_suggestions("k\u{f6}pekler").unwrap().len(), 1, "the cache is rebuilt with NFC keys; the log still holds NFD");
    fs::remove_dir_all(root).ok();
}

#[test]
fn reapplying_an_unchanged_analysis_typed_decomposed_writes_nothing() {
    let root = temp_root("nfc-reapply");
    let mut p = nfc_project(&root);
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, NFC_CAY).unwrap().0;
    let tok = p.tokenize(&txt, &seg, &[NFC_CAY]).unwrap().0.remove(0);
    let spec = |surface: &str| wugs_core::MorphSpec { surface: surface.into(), morph_type: "root".into(), gloss: Some("tea".into()), ..Default::default() };
    p.apply_analysis(&txt, &tok, &[spec(NFC_CAY)]).unwrap();
    let before = p.log.read_all().unwrap().ops.len();
    p.apply_analysis(&txt, &tok, &[spec(NFD_CAY)]).unwrap();
    let after = p.log.read_all().unwrap().ops;
    assert_eq!(after.len(), before, "no ops for an unchanged analysis: {:?}", after[before..].iter().map(|o| (&o.verb, &o.field)).collect::<Vec<_>>());
    fs::remove_dir_all(root).ok();
}
