//! TEI in, then out as CoNLL-U and as TEI again.

use std::path::PathBuf;

use wugs_conllu::{format, ExportSentence};
use wugs_core::Project;
use wugs_tei::{export_tei, import_tei};

const SESSION: &str = include_str!("fixtures/session.xml");

fn temp_project() -> (PathBuf, Project, String) {
    let root = std::env::temp_dir().join(format!("wugs-tei-test-{}", wugs_model::ulid()));
    let mut p = Project::open(&root, "prs_Tester").unwrap();
    let tier = p.add_tier("sentences", "time-aligned", Some("transcription"), None).unwrap().0;
    (root, p, tier)
}

fn sentences(p: &Project, text: &str) -> Vec<ExportSentence> {
    p.proj.ordered_children("text", text, "segment").unwrap().into_iter().map(|segment| ExportSentence { segment, translations: Vec::new() }).collect()
}

fn contents(p: &Project, text: &str) -> Vec<String> {
    sentences(p, text).iter().map(|s| p.proj.field_str(&s.segment, "content").unwrap().unwrap_or_default()).collect()
}

#[test]
fn sentences_words_and_tags_come_in() {
    let (root, mut p, tier) = temp_project();
    let r = import_tei(&mut p, SESSION, "session", &tier).unwrap();
    assert_eq!(r.texts.len(), 1);
    assert_eq!(p.proj.field_str(&r.texts[0], "title").unwrap().as_deref(), Some("Sesión de prueba"));
    let meta = p.text_metadata(&r.texts[0]).unwrap();
    let get = |k: &str| meta.iter().find(|(m, _)| m == k).map(|(_, v)| v.as_str());
    assert_eq!(get("author"), Some("Comisión Inventada"));
    assert_eq!(get("date"), Some("2021-03-04"), "the creation date");
    assert_eq!(get("genre"), Some("parla.debate"));
    assert_eq!(get("keywords"), Some("pruebas"));
    assert_eq!(get("language"), Some("es"));
    assert_eq!(get("licence"), Some("https://creativecommons.org/licenses/by/4.0/"));
    assert_eq!(get("speakers"), Some("Ana Ejemplo; Luis"));
    assert_eq!(contents(&p, &r.texts[0]), ["Buenos días.", "Empezamos ya.", "Gracias", "Un párrafo sin frases marcadas."]);
    assert_eq!((r.segments, r.words, r.blocks_as_segments), (4, 7, 1));
    assert_eq!(r.not_carried.get("note"), Some(&2), "{:?}", r.not_carried);
    let segs = sentences(&p, &r.texts[0]);
    assert_eq!(p.proj.field_str(&segs[0].segment, "x.tei.who").unwrap().as_deref(), Some("Ana"));
    assert_eq!(p.proj.field_str(&segs[0].segment, "x.tei.id").unwrap().as_deref(), Some("u1.s1"));
    assert!(p.proj.is_punct_token(&p.proj.members(&segs[0].segment).unwrap()[2]));

    // The tags reach CoNLL-U: UPosTag as UPOS, the rest of msd as FEATS,
    // pos as UPOS when it is a UD tag, and a tagset code as XPOS.
    let ud = wugs_conllu::export(&p, "x", &segs).unwrap().conllu;
    let s = format::parse(&ud).unwrap();
    let w: Vec<&format::Cols> = s[0].words().map(|(_, c)| c).collect();
    assert_eq!((w[0][format::LEMMA].as_str(), w[0][format::UPOS].as_str(), w[0][format::FEATS].as_str()), ("buen", "ADJ", "Gender=Masc|Number=Plur"));
    assert_eq!(w[1][format::MISC], "SpaceAfter=No");
    assert_eq!(w[2][format::UPOS], "PUNCT");
    let e: Vec<&format::Cols> = s[1].words().map(|(_, c)| c).collect();
    assert_eq!((e[0][format::UPOS].as_str(), e[0][format::FEATS].as_str()), ("VERB", "Mood=Ind|Person=1"));
    let g: Vec<&format::Cols> = s[2].words().map(|(_, c)| c).collect();
    assert_eq!((g[0][format::UPOS].as_str(), g[0][format::XPOS].as_str()), ("_", "Ncfp000"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn the_same_file_again_adds_nothing_and_a_corpus_gives_a_text_each() {
    let (root, mut p, tier) = temp_project();
    import_tei(&mut p, SESSION, "session", &tier).unwrap();
    let again = import_tei(&mut p, SESSION, "session", &tier).unwrap();
    assert!(again.texts.is_empty() && again.matched.len() == 1);
    let body = |t: &str| format!("<TEI><teiHeader><fileDesc><titleStmt><title>{t}</title></titleStmt></fileDesc></teiHeader><text><body><p>{t} text.</p></body></text></TEI>");
    let corpus = format!("<teiCorpus>{}{}</teiCorpus>", body("One"), body("Two"));
    let r = import_tei(&mut p, &corpus, "c", &tier).unwrap();
    assert_eq!(r.texts.len(), 2);
    assert!(import_tei(&mut p, "<html><body/></html>", "x", &tier).unwrap_err().contains("not TEI"));
    assert!(import_tei(&mut p, "<TEI><text>", "x", &tier).unwrap_err().contains("well-formed"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn tei_goes_out_and_comes_back_the_same() {
    let (root, mut p, tier) = temp_project();
    let text = import_tei(&mut p, SESSION, "session", &tier).unwrap().texts[0].clone();
    p.set_text_metadata(&text, "Recorded by", "Ana & co").unwrap();
    let out = export_tei(&p, "Sesión de prueba", Some(("Spanish", "es")), &sentences(&p, &text), "wugs test").unwrap();
    assert!(out.tei.contains("<s xml:id=\"u1.s1\" who=\"#Ana\">"), "{}", out.tei);
    assert!(out.tei.contains("<w lemma=\"día\" pos=\"NOUN\" msd=\"Gender=Masc|Number=Plur\" join=\"right\">días</w>"), "{}", out.tei);
    assert!(out.tei.contains("<language ident=\"es\">Spanish</language>"));
    wugs_xml::parse(&out.tei).unwrap();
    // Back in, as a second project: the same sentences, tags and spacing.
    let (root2, mut q, tier2) = temp_project();
    let back = import_tei(&mut q, &out.tei, "x", &tier2).unwrap().texts[0].clone();
    assert_eq!(contents(&q, &back), contents(&p, &text));
    assert_eq!(q.text_metadata(&back).unwrap(), p.text_metadata(&text).unwrap(), "the header carries every key back: {}", out.tei);
    let a = wugs_conllu::export(&p, "x", &sentences(&p, &text)).unwrap().conllu;
    let b = wugs_conllu::export(&q, "x", &sentences(&q, &back)).unwrap().conllu;
    let cols = |s: &str| format::parse(s).unwrap().iter().flat_map(|s| s.words().map(|(_, c)| c[..5].to_vec()).collect::<Vec<_>>()).collect::<Vec<_>>();
    assert_eq!(cols(&a), cols(&b), "form, lemma, UPOS, XPOS and FEATS survive TEI");
    std::fs::remove_dir_all(root).ok();
    std::fs::remove_dir_all(root2).ok();
}
