//! Glossing intents (GLOSSING-MODEL.md), driven through the facade.

use std::fs;
use std::path::PathBuf;

use std::collections::BTreeMap;

use wugs_core::{MorphSpec, Project};
use wugs_model::Op;

fn temp() -> (PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("wugs-gloss-test-{}", wugs_model::ulid()));
    let p = Project::open(&root, "prs_T").unwrap();
    (root, p)
}

/// A segment glossed like DATA-MODEL §11, with a second-language gloss and a sense link:
/// köpek(root, dog, es perro, sense) -ler(suffix, PL) | gel(root, come) -di(suffix, PST).
struct Fixture {
    root: PathBuf,
    p: Project,
    txt: String,
    seg: String,
    toks: Vec<String>,
    sense: String,
}

fn glossed() -> Fixture {
    let (root, mut p) = temp();
    let (_, sense) = p.create_lexeme("köpek", "dog").unwrap();
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "köpekler geldi").unwrap().0;
    let toks = p.tokenize(&txt, &seg, &["köpekler", "geldi"]).unwrap().0;
    let m = p.split_morph(&txt, &toks[0], &[("köpek", "root"), ("ler", "suffix")]).unwrap().0;
    p.link_sense(&txt, &m[0], &sense).unwrap();
    p.set_gloss(&txt, &m[0], "dog", None).unwrap();
    p.set_gloss(&txt, &m[0], "perro", Some("es")).unwrap();
    p.set_gloss(&txt, &m[1], "PL", None).unwrap();
    let m = p.split_morph(&txt, &toks[1], &[("gel", "root"), ("di", "suffix")]).unwrap().0;
    p.set_gloss(&txt, &m[0], "come", None).unwrap();
    p.set_gloss(&txt, &m[1], "PST", None).unwrap();
    Fixture { root, p, txt, seg, toks, sense }
}

type Row = (String, String, Option<String>, Option<String>, Option<String>);

/// A token's live morphs in order: surface, type, gloss, gloss:es, sense.
fn analysis(p: &Project, tok: &str) -> Vec<Row> {
    let f = |m: &str, k: &str| p.proj.field_str(m, k).unwrap();
    p.proj
        .members(tok)
        .unwrap()
        .iter()
        .map(|m| (f(m, "surface").unwrap(), f(m, "type").unwrap(), f(m, "gloss"), f(m, "gloss:es"), f(m, "sense")))
        .collect()
}

fn live_tokens(p: &Project, seg: &str) -> Vec<(String, String)> {
    p.proj
        .members(seg)
        .unwrap()
        .into_iter()
        .map(|t| (p.proj.field_str(&t, "form").unwrap().unwrap(), t))
        .collect()
}

fn txn_ops(p: &Project, txn: &str) -> Vec<Op> {
    p.log.read_all().unwrap().ops.into_iter().filter(|o| o.txn == txn).collect()
}

#[test]
fn retokenize_with_unchanged_forms_keeps_every_token_and_writes_nothing() {
    let mut f = glossed();
    let before: Vec<_> = f.toks.iter().map(|t| analysis(&f.p, t)).collect();

    let r = f.p.retokenize(&f.txt, &f.seg, &["köpekler", "geldi"]).unwrap();
    assert_eq!((r.carried, r.lost_analysed), (2, 0));
    assert_eq!(r.tokens, f.toks, "the same tokens, not copies");
    assert_eq!(r.commit.ops, 0, "nothing changed, nothing written");
    let after: Vec<_> = r.tokens.iter().map(|t| analysis(&f.p, t)).collect();
    assert_eq!(after, before);
    assert_eq!(after[0][0].4.as_deref(), Some(f.sense.as_str()));
    fs::remove_dir_all(&f.root).ok();
}

#[test]
fn a_case_change_keeps_the_token_and_sets_only_its_form() {
    let mut f = glossed();
    let before = analysis(&f.p, &f.toks[0]);
    let r = f.p.retokenize(&f.txt, &f.seg, &["Köpekler", "geldi"]).unwrap();
    assert_eq!((r.carried, r.lost_analysed), (2, 0), "a sentence-initial capital is the same word");
    assert_eq!(r.tokens, f.toks);
    assert_eq!(analysis(&f.p, &f.toks[0]), before);
    let ops = txn_ops(&f.p, &r.commit.txn);
    assert_eq!(ops.len(), 1, "{ops:?}");
    assert_eq!((ops[0].verb.as_str(), ops[0].field.as_deref(), ops[0].value.as_str()), ("set", Some("form"), Some("Köpekler")));
    assert_eq!(ops[0].intent.as_deref(), Some("retokenize"));
    fs::remove_dir_all(&f.root).ok();
}

#[test]
fn a_changed_word_loses_its_analysis_and_the_rest_keeps_theirs() {
    let mut f = glossed();
    let kept = analysis(&f.p, &f.toks[0]);
    let r = f.p.retokenize(&f.txt, &f.seg, &["köpekler", "gitti"]).unwrap();
    assert_eq!((r.carried, r.lost_analysed), (1, 1));
    assert_eq!(analysis(&f.p, &r.tokens[0]), kept);
    assert!(analysis(&f.p, &r.tokens[1]).is_empty(), "gitti starts bare");
    assert_eq!(live_tokens(&f.p, &f.seg).iter().map(|(form, _)| form.as_str()).collect::<Vec<_>>(), ["köpekler", "gitti"]);
    assert_eq!(r.tokens[0], f.toks[0]);
    assert!(!f.p.proj.is_tombstoned(&f.toks[1]).unwrap(), "the dropped token is hidden, not deleted");
    let ops = txn_ops(&f.p, &r.commit.txn);
    assert!(ops.iter().all(|o| o.verb != "delete"), "{ops:?}");
    assert_eq!(ops.iter().filter(|o| o.field.as_deref() == Some("tokens")).count(), 1, "the list is written once");

    let Fixture { root, p, seg, .. } = f;
    let live = live_tokens(&p, &seg);
    drop(p);
    fs::remove_dir_all(root.join("cache")).unwrap();
    let rebuilt = Project::open(&root, "prs_T").unwrap();
    assert_eq!(live_tokens(&rebuilt, &seg), live, "replay agrees on which tokens are live");
    fs::remove_dir_all(root).ok();
}

#[test]
fn inserting_or_deleting_a_word_mid_utterance_keeps_the_others() {
    let mut f = glossed();
    let (first, last) = (analysis(&f.p, &f.toks[0]), analysis(&f.p, &f.toks[1]));

    let r = f.p.retokenize(&f.txt, &f.seg, &["köpekler", "dün", "geldi"]).unwrap();
    assert_eq!((r.carried, r.lost_analysed), (2, 0));
    assert_eq!(analysis(&f.p, &r.tokens[0]), first);
    assert!(analysis(&f.p, &r.tokens[1]).is_empty(), "the new word is bare");
    assert_eq!(analysis(&f.p, &r.tokens[2]), last, "geldi moved from position 1 to 2 and kept its analysis");

    let r = f.p.retokenize(&f.txt, &f.seg, &["köpekler", "geldi"]).unwrap();
    assert_eq!((r.carried, r.lost_analysed), (2, 0), "deleting the inserted word loses nothing");
    assert_eq!(analysis(&f.p, &r.tokens[1]), last);

    let r = f.p.retokenize(&f.txt, &f.seg, &["geldi"]).unwrap();
    assert_eq!((r.carried, r.lost_analysed), (1, 1), "deleting an analysed word drops only its analysis");
    assert_eq!(analysis(&f.p, &r.tokens[0]), last);
    assert_eq!(live_tokens(&f.p, &f.seg).len(), 1);

    assert!(f.p.retokenize(&f.txt, &f.seg, &["a", " "]).is_err(), "an empty form is refused");
    fs::remove_dir_all(&f.root).ok();
}

#[test]
fn glosses_in_other_languages_are_sibling_fields_that_search_finds() {
    let (root, mut p) = temp();
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "köpekler").unwrap().0;
    let toks = p.tokenize(&txt, &seg, &["köpekler"]).unwrap().0;
    let morphs = p.split_morph(&txt, &toks[0], &[("köpek", "root"), ("ler", "suffix")]).unwrap().0;

    p.set_gloss(&txt, &morphs[0], "dog", None).unwrap();
    p.set_gloss(&txt, &morphs[0], "perro", Some("es")).unwrap();
    p.set_gloss(&txt, &morphs[0], "Hund", Some("de-CH")).unwrap();

    let field = |m: &str, f: &str| p.proj.field_str(m, f).unwrap();
    assert_eq!(field(&morphs[0], "gloss").as_deref(), Some("dog"), "the primary gloss is untouched");
    assert_eq!(field(&morphs[0], "gloss:es").as_deref(), Some("perro"));
    assert_eq!(field(&morphs[0], "gloss:de-ch").as_deref(), Some("Hund"), "tags are stored lowercase");

    let hits = p.proj.search("perro", &[], 10).unwrap();
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!((hits[0].kind.as_str(), hits[0].field.as_str()), ("morph", "gloss:es"));

    p.set_gloss(&txt, &morphs[0], "can", Some("ES")).unwrap();
    assert!(p.proj.search("perro", &[], 10).unwrap().is_empty(), "an edit replaces the indexed text");
    assert_eq!(p.proj.search("can", &[], 10).unwrap().len(), 1, "the tag is case-insensitive, one field per language");

    for bad in ["", "e", "e_s", "es es", "a".repeat(36).as_str()] {
        assert!(p.set_gloss(&txt, &morphs[0], "x", Some(bad)).is_err(), "{bad:?}");
    }
    assert!(p.set_gloss(&txt, &morphs[0], "x", Some(&"a".repeat(35))).is_ok(), "35 characters is the limit");

    drop(p);
    fs::remove_dir_all(root.join("cache")).unwrap();
    let p2 = Project::open(&root, "prs_T").unwrap();
    assert_eq!(p2.proj.field_str(&morphs[0], "gloss:es").unwrap().as_deref(), Some("can"), "replay keeps the colon field");
    assert_eq!(p2.proj.search("can", &[], 10).unwrap().len(), 1, "replay rebuilds the index for it");
    fs::remove_dir_all(root).ok();
}

#[test]
fn tokenize_and_split_morph_return_their_findings() {
    let (root, mut p) = temp();
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "köpekler").unwrap().0;
    let (toks, res) = p.tokenize(&txt, &seg, &["köpekler"]).unwrap();
    assert_eq!((toks.len(), res.ops), (1, 2), "the token and the segment's token list");
    assert!(res.findings.is_empty(), "{:?}", res.findings);

    let (_, res) = p.split_morph(&txt, &toks[0], &[("köpek", "root"), ("lar", "suffix")]).unwrap();
    assert!(
        res.findings.iter().any(|f| f.code == "morph-concat"),
        "a surface typo is reported, not blocked: {:?}",
        res.findings
    );
    assert!(res.findings.iter().any(|f| f.code == "gloss-missing"), "{:?}", res.findings);
    fs::remove_dir_all(root).ok();
}

fn spec(surface: &str, ty: &str, gloss: &str) -> MorphSpec {
    MorphSpec { surface: surface.into(), morph_type: ty.into(), gloss: Some(gloss.into()), ..Default::default() }
}

#[test]
fn apply_analysis_diffs_against_the_current_morphs_in_one_txn() {
    let mut f = glossed();
    let old = f.p.proj.members(&f.toks[0]).unwrap();
    let mut dog = spec("köpek", "root", "dog");
    dog.glosses_by_lang = BTreeMap::from([("es".to_string(), "perro".to_string())]);
    dog.sense = Some(f.sense.clone());
    let (ids, res) = f.p.apply_analysis(&f.txt, &f.toks[0], &[dog.clone(), spec("ler", "suffix", "PL"), spec("∅", "suffix", "NOM")]).unwrap();

    assert_eq!(ids[..2], old[..], "unchanged morphs keep their ids");
    let live = analysis(&f.p, &f.toks[0]);
    assert_eq!(live.len(), 3, "{live:?}");
    assert_eq!(live[0], ("köpek".into(), "root".into(), Some("dog".into()), Some("perro".into()), Some(f.sense.clone())));
    assert_eq!(live[2].0, "∅");
    assert!(res.findings.iter().all(|x| x.code != "morph-concat"), "a zero morph adds no letters: {:?}", res.findings);
    let ops = txn_ops(&f.p, &res.txn);
    assert_eq!(ops.len(), 2, "one new morph and the token's list: {ops:?}");
    assert!(ops.iter().all(|o| o.intent.as_deref() == Some("apply-analysis") && o.scope == f.txt));

    let written = |p: &Project, txn: &str| -> Vec<(String, String)> {
        txn_ops(p, txn).iter().map(|o| (o.target.clone(), o.field.clone().unwrap_or_default())).collect()
    };
    let (_, res) = f.p.apply_analysis(&f.txt, &f.toks[0], &[dog.clone(), spec("lar", "suffix", "PL"), spec("∅", "suffix", "NOM")]).unwrap();
    assert_eq!(written(&f.p, &res.txn), [(old[1].clone(), "surface".to_string())], "a corrected surface keeps its morph and only that field is set");

    let (_, res) = f.p.apply_analysis(&f.txt, &f.toks[0], &[dog, spec("lar", "suffix", "PL")]).unwrap();
    assert_eq!(written(&f.p, &res.txn), [(f.toks[0].clone(), "morphs".to_string())], "dropping ∅ writes only the list");
    assert!(!f.p.proj.is_tombstoned(&ids[2]).unwrap(), "a morph left out is hidden, not deleted");

    let (ids, _) = f.p.apply_analysis(&f.txt, &f.toks[0], &[]).unwrap();
    assert!(ids.is_empty() && analysis(&f.p, &f.toks[0]).is_empty(), "an empty analysis clears the token");

    assert!(f.p.apply_analysis(&f.txt, &f.toks[0], &[spec("", "root", "x")]).is_err());
    let mut bad = spec("a", "root", "x");
    bad.glosses_by_lang.insert("not a tag".into(), "y".into());
    assert!(f.p.apply_analysis(&f.txt, &f.toks[0], &[bad]).is_err());
    assert!(f.p.apply_analysis(&f.txt, &f.seg, &[]).is_err(), "only a token takes an analysis");
    fs::remove_dir_all(&f.root).ok();
}

#[test]
fn apply_to_unanalysed_never_touches_an_analysed_token() {
    let (root, mut p) = temp();
    let add = |p: &mut Project, title: &str, forms: &[&str]| {
        let txt = p.create_text(title).unwrap();
        let seg = p.add_segment(&txt, "tir_utt", 0, None, None, &forms.join(" ")).unwrap().0;
        let toks = p.tokenize(&txt, &seg, forms).unwrap().0;
        (txt, seg, toks)
    };
    let (t1, _, a) = add(&mut p, "one", &["wugs", "Wugs"]);
    let (t2, _, b) = add(&mut p, "two", &["WUGS", "dax"]);
    let (t3, seg3, c) = add(&mut p, "gone", &["wugs"]);

    p.apply_analysis(&t1, &a[0], &[spec("wug", "root", "wug"), spec("s", "suffix", "PL")]).unwrap();
    let mine = analysis(&p, &a[0]);
    let mine_ids = p.proj.members(&a[0]).unwrap();
    p.delete_segment(&t3, &seg3).unwrap();

    assert_eq!(p.unanalysed_with_form("wugs").unwrap().len(), 2, "case folds, the analysed and the deleted-segment tokens are not counted");

    let new = [spec("wug", "root", "wug"), spec("s", "suffix", "PLURAL")];
    let r = p.apply_analysis_to_unanalysed("WuGs", &new).unwrap();
    assert_eq!((r.applied, r.texts, r.skipped_analysed), (2, 2, 1));
    assert_eq!(analysis(&p, &a[0]), mine, "the analysed token keeps its own analysis");
    assert_eq!(p.proj.members(&a[0]).unwrap(), mine_ids, "its morphs were not even re-created");
    assert!(analysis(&p, &a[1]).iter().all(|m| m.2.as_deref() != Some("PL")));
    assert_eq!(analysis(&p, &a[1])[1].2.as_deref(), Some("PLURAL"));
    assert_eq!(analysis(&p, &a[1])[0].0, "Wug", "«Wugs» takes its own capital, not the applied lower case");
    assert_eq!(analysis(&p, &b[0]).len(), 2);
    assert_eq!(analysis(&p, &b[0])[0].0, "Wug", "«WUGS» takes its first character; the rest is the analysis's");
    assert!(analysis(&p, &b[1]).is_empty(), "other forms are untouched");
    assert!(analysis(&p, &c[0]).is_empty(), "a token in a deleted segment is untouched");

    let applied: Vec<Op> = p.log.read_all().unwrap().ops.into_iter().filter(|o| o.intent.as_deref() == Some("apply-analysis") && o.verb == "create" && o.entity == "morph").collect();
    let txns: std::collections::BTreeSet<_> = applied.iter().map(|o| (o.txn.clone(), o.scope.clone())).collect();
    assert_eq!(txns.len(), 3, "one txn per text, plus the earlier single apply: {txns:?}");
    assert!(txns.iter().any(|(_, s)| *s == t2) && txns.iter().filter(|(_, s)| *s == t1).count() == 2);

    let again = p.apply_analysis_to_unanalysed("wugs", &new).unwrap();
    assert_eq!((again.applied, again.skipped_analysed), (0, 3), "a second run has nothing left to do");
    assert!(p.apply_analysis_to_unanalysed("wugs", &[]).is_err());
    fs::remove_dir_all(root).ok();
}

#[test]
fn token_findings_report_what_a_commit_would_have() {
    let mut f = glossed();
    assert!(f.p.token_findings(&f.toks).unwrap().is_empty(), "a fully glossed segment is clean");

    f.p.apply_analysis(&f.txt, &f.toks[1], &[spec("gel", "root", "come"), MorphSpec { surface: "dı".into(), morph_type: "suffix".into(), ..Default::default() }]).unwrap();
    let found = f.p.token_findings(&f.toks).unwrap();
    let codes: Vec<_> = found.iter().map(|x| (x.code, x.target.as_str())).collect();
    assert!(codes.contains(&("morph-concat", f.toks[1].as_str())), "{codes:?}");
    assert!(codes.iter().any(|(c, _)| *c == "gloss-missing"), "{codes:?}");
    assert!(found.iter().all(|x| x.severity == wugs_validate::Severity::Warn), "findings never block");
    assert!(f.p.token_findings(&f.toks[..1]).unwrap().is_empty(), "only the tokens asked about");
    fs::remove_dir_all(&f.root).ok();
}

#[test]
fn suggestions_group_identical_analyses_across_texts_and_rank_by_count() {
    let (root, mut p) = temp();
    let add = |p: &mut Project, title: &str, forms: &[&str]| {
        let txt = p.create_text(title).unwrap();
        let seg = p.add_segment(&txt, "tir_utt", 0, None, None, &forms.join(" ")).unwrap().0;
        let toks = p.tokenize(&txt, &seg, forms).unwrap().0;
        (txt, seg, toks)
    };
    let pl = [spec("köpek", "root", "dog"), spec("ler", "suffix", "PL")];
    let plural = [spec("köpek", "root", "dog"), spec("ler", "suffix", "PLURAL")];

    let (t1, _, a) = add(&mut p, "one", &["köpekler", "Köpekler", "dün"]);
    let (t2, _, b) = add(&mut p, "two", &["KÖPEKLER", "köpekler"]);
    let (t3, _, c) = add(&mut p, "three", &["köpekler", "köpekler"]);
    let (t4, seg4, d) = add(&mut p, "gone", &["köpekler"]);
    p.apply_analysis(&t1, &a[0], &pl).unwrap();
    p.apply_analysis(&t1, &a[1], &pl).unwrap();
    p.apply_analysis(&t2, &b[0], &plural).unwrap();
    p.apply_analysis(&t3, &c[0], &plural).unwrap();
    p.apply_analysis(&t3, &c[1], &plural).unwrap();
    p.apply_analysis(&t1, &a[2], &[spec("dün", "root", "yesterday")]).unwrap();
    p.apply_analysis(&t4, &d[0], &pl).unwrap();
    p.delete_segment(&t4, &seg4).unwrap();

    let got = p.analysis_suggestions("Köpekler").unwrap();
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!((got[0].count, got[0].morphs.clone()), (3, plural.to_vec()), "PLURAL: 3 uses, ranked first");
    assert_eq!((got[1].count, got[1].morphs.clone()), (2, pl.to_vec()), "PL: 2 uses; the deleted segment's token does not count");
    assert!(p.analysis_suggestions("kedi").unwrap().is_empty());
    assert_eq!(p.analysis_suggestions("dün").unwrap().len(), 1, "other forms are separate");
    assert!(analysis(&p, &b[1]).is_empty(), "the unanalysed token is not a suggestion");

    let mut dog = spec("köpek", "root", "dog");
    dog.glosses_by_lang.insert("es".into(), "perro".into());
    p.apply_analysis(&t2, &b[1], &[dog.clone()]).unwrap();
    let got = p.analysis_suggestions("köpekler").unwrap();
    assert_eq!(got.len(), 3);
    assert_eq!(got[2].morphs, vec![dog], "extra-language glosses are part of an analysis's identity");
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_suggestion_never_links_a_deleted_sense() {
    let mut f = glossed();
    let mut t = f.p.intent("delete-sense");
    t.delete("sense", &f.sense, "lexicon");
    t.commit().unwrap();
    let got = f.p.analysis_suggestions("köpekler").unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].morphs[0].sense, None, "the tombstoned sense is dropped from the suggestion");
    assert_eq!(got[0].morphs[0].gloss.as_deref(), Some("dog"));
    fs::remove_dir_all(&f.root).ok();
}

#[test]
fn lexicon_lookup_finds_lemmas_and_allomorph_forms_one_match_per_sense() {
    let (root, mut p) = temp();
    let (dog, dog_sense) = p.create_lexeme("köpek", "dog").unwrap();
    let (ler, _) = p.create_lexeme("-ler", "PL").unwrap();
    let (_, bark_sense) = p.create_lexeme("köpek", "bark").unwrap();
    let mut t = p.intent("add-allomorph");
    t.create("form", "frm", "lexicon", vec![("lexeme", wugs_model::Value::str(&dog)), ("kind", wugs_model::Value::str("allomorph")), ("form", wugs_model::Value::str("köpeğ"))]);
    t.commit().unwrap();

    let by_lemma = p.lexicon_lookup("KÖPEK").unwrap();
    let senses: Vec<_> = by_lemma.iter().map(|m| (m.sense.as_str(), m.gloss.as_str(), m.lemma.as_str())).collect();
    assert_eq!(senses, [(dog_sense.as_str(), "dog", "köpek"), (bark_sense.as_str(), "bark", "köpek")], "homographs both offered, case folded");
    let by_form = p.lexicon_lookup("köpeğ").unwrap();
    assert_eq!(by_form.len(), 1, "{by_form:?}");
    assert_eq!((by_form[0].lexeme.as_str(), by_form[0].sense.as_str(), by_form[0].lemma.as_str()), (dog.as_str(), dog_sense.as_str(), "köpek"));
    let affix = p.lexicon_lookup("ler").unwrap();
    assert_eq!((affix.len(), affix[0].lexeme.as_str(), affix[0].gloss.as_str()), (1, ler.as_str(), "PL"), "the lemma -ler answers to ler");
    assert!(p.lexicon_lookup("kedi").unwrap().is_empty());

    let mut t = p.intent("delete-lexeme");
    t.delete("lexeme", &dog, "lexicon");
    t.commit().unwrap();
    assert_eq!(p.lexicon_lookup("köpek").unwrap().len(), 1, "a deleted lexeme is not offered, nor is its allomorph");
    assert!(p.lexicon_lookup("köpeğ").unwrap().is_empty());
    fs::remove_dir_all(root).ok();
}

/// Cost of the derived queries at 100k tokens, with loose bounds. Ignored by
/// default (building the 100k-token project takes about a minute in a
/// release build); run with
/// `cargo test --release -p wugs-core --test glossing -- --ignored --nocapture`.
#[test]
#[ignore]
fn suggestion_queries_stay_fast_at_100k_tokens() {
    use std::time::Instant;
    use wugs_model::Value;
    let (root, mut p) = temp();
    let txt = p.create_text("big").unwrap();
    let built = Instant::now();
    let mut t = p.intent("bulk");
    let mut seg = String::new();
    for i in 0..100_000usize {
        if i % 10 == 0 {
            seg = t.create("segment", "seg", &txt, vec![("text", Value::str(&txt)), ("tier", Value::str("tir_utt")), ("pos", Value::str(&wugs_model::pos_seq(i / 10)))]);
        }
        let form = if i % 50 == 0 { "the".to_string() } else { format!("w{}", (i * 7919) % 5000) };
        let tok = t.create("token", "tok", &txt, vec![("segment", Value::str(&seg)), ("pos", Value::str(&wugs_model::pos_seq(i % 10))), ("form", Value::str(&form))]);
        if i % 100 != 1 {
            for (k, (surface, ty)) in [(form.as_str(), "root"), ("x", "suffix")].into_iter().enumerate() {
                let gloss = format!("g{}", i % 3);
                t.create("morph", "mor", &txt, vec![("token", Value::str(&tok)), ("pos", Value::str(&wugs_model::pos_seq(k))), ("surface", Value::str(surface)), ("type", Value::str(ty)), ("gloss", Value::str(&gloss))]);
            }
        }
    }
    t.commit().unwrap();
    let mut t = p.intent("lex");
    for n in 0..5000 {
        let lex = t.create("lexeme", "lex", "lexicon", vec![("lemma", Value::str(&format!("w{n}")))]);
        t.create("sense", "sen", "lexicon", vec![("lexeme", Value::str(&lex)), ("gloss", Value::str("g")), ("pos", Value::str(&wugs_model::pos_seq(0)))]);
    }
    t.commit().unwrap();
    println!("built 100k tokens in {:?}", built.elapsed());
    // Bounds are ~10x the release-build timings measured on 2026-11-09, so
    // they catch a lost index (seconds), not jitter.
    let time = |label: &str, bound_ms: u128, f: &mut dyn FnMut() -> usize| {
        let start = Instant::now();
        let n = f();
        let took = start.elapsed();
        println!("{label}: {n} results in {took:?}");
        assert!(took.as_millis() < bound_ms, "{label} took {took:?}, bound {bound_ms} ms");
    };
    time("analysis_suggestions(the)  [2000 occurrences]", 2000, &mut || p.analysis_suggestions("the").unwrap().len());
    time("analysis_suggestions(w17)  [~20 occurrences]", 50, &mut || p.analysis_suggestions("w17").unwrap().len());
    time("unanalysed_with_form(w17)", 20, &mut || p.unanalysed_with_form("w17").unwrap().len());
    let words: Vec<String> = (0..10).map(|n| format!("w{n}")).collect();
    time("unanalysed_counts(10 forms)", 50, &mut || p.unanalysed_counts(&words).unwrap().len());
    time("lexicon_lookup(w17) [5000 lexemes]", 20, &mut || p.lexicon_lookup("w17").unwrap().len());
    fs::remove_dir_all(root).ok();
}

#[test]
fn turkish_dotted_and_dotless_i_find_each_others_analyses() {
    let (root, mut p) = temp();
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "ışık İstanbul Işık istanbul").unwrap().0;
    let toks = p.tokenize(&txt, &seg, &["ışık", "İstanbul", "Işık", "istanbul"]).unwrap().0;
    p.apply_analysis(&txt, &toks[0], &[spec("ışık", "root", "light")]).unwrap();
    p.apply_analysis(&txt, &toks[1], &[spec("İstanbul", "root", "Istanbul")]).unwrap();

    let light = p.analysis_suggestions("Işık").unwrap();
    assert_eq!((light.len(), light[0].morphs[0].gloss.as_deref()), (1, Some("light")), "Işık finds ışık");
    let city = p.analysis_suggestions("istanbul").unwrap();
    assert_eq!((city.len(), city[0].morphs[0].gloss.as_deref()), (1, Some("Istanbul")), "istanbul finds İstanbul");
    assert_eq!(p.unanalysed_with_form("IŞIK").unwrap(), [(txt.clone(), toks[2].clone())]);
    assert_eq!(p.unanalysed_with_form("İSTANBUL").unwrap(), [(txt.clone(), toks[3].clone())]);
    fs::remove_dir_all(root).ok();
}

#[test]
fn picking_a_sense_links_and_glosses_in_one_txn() {
    let mut f = glossed();
    let morph = f.p.proj.members(&f.toks[1]).unwrap().remove(0);
    let (_, sense) = f.p.create_lexeme("gel", "come").unwrap();
    let res = f.p.pick_sense(&f.txt, &morph, &sense, "come").unwrap();
    let ops = txn_ops(&f.p, &res.txn);
    let fields: Vec<_> = ops.iter().map(|o| o.field.as_deref().unwrap_or("")).collect();
    assert_eq!(fields, ["sense", "gloss"]);
    assert!(ops.iter().all(|o| o.intent.as_deref() == Some("pick-sense")));
    assert!(f.p.pick_sense(&f.txt, &morph, &f.toks[0], "x").is_err(), "only a sense can be picked");
    fs::remove_dir_all(&f.root).ok();
}

#[test]
fn unanalysed_counts_answer_every_word_in_one_call() {
    let (root, mut p) = temp();
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "wugs Wugs dax wugs").unwrap().0;
    let toks = p.tokenize(&txt, &seg, &["wugs", "Wugs", "dax", "wugs"]).unwrap().0;
    p.apply_analysis(&txt, &toks[0], &[spec("wugs", "root", "wugs")]).unwrap();
    let counts = p.unanalysed_counts(&["WUGS".to_string(), "dax".to_string(), "zap".to_string()]).unwrap();
    assert_eq!(counts, [2, 1, 0], "case folds; the analysed one does not count");
    fs::remove_dir_all(root).ok();
}
