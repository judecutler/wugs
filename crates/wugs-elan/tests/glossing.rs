//! ELAN glossing tiers (words, morphs, glosses) read as tokens and morphs.

use std::fs;

use wugs_core::Project;
use wugs_elan::glossing::{layouts, read_glossing};
use wugs_elan::{export_eaf, import_eaf};

fn temp_project() -> (std::path::PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("wugs-elan-gloss-{}", wugs_model::ulid()));
    let p = Project::open(&root, "prs_Tester").unwrap();
    (root, p)
}

/// A Toolbox-style template: utterance > words (Symbolic_Subdivision) >
/// morphs (Symbolic_Subdivision) > glosses and part of speech (Symbolic_Association),
/// and a free translation. Synthetic data.
const GLOSSED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ANNOTATION_DOCUMENT AUTHOR="" DATE="2026-01-01T00:00:00+00:00" FORMAT="3.0" VERSION="3.0">
<HEADER MEDIA_FILE="" TIME_UNITS="milliseconds"/>
<TIME_ORDER><TIME_SLOT TIME_SLOT_ID="ts1" TIME_VALUE="0"/><TIME_SLOT TIME_SLOT_ID="ts2" TIME_VALUE="2000"/><TIME_SLOT TIME_SLOT_ID="ts3" TIME_VALUE="3500"/></TIME_ORDER>
<TIER LINGUISTIC_TYPE_REF="utt" TIER_ID="tx@A">
<ANNOTATION><ALIGNABLE_ANNOTATION ANNOTATION_ID="u1" TIME_SLOT_REF1="ts1" TIME_SLOT_REF2="ts2"><ANNOTATION_VALUE>nikanira tela</ANNOTATION_VALUE></ALIGNABLE_ANNOTATION></ANNOTATION>
<ANNOTATION><ALIGNABLE_ANNOTATION ANNOTATION_ID="u2" TIME_SLOT_REF1="ts2" TIME_SLOT_REF2="ts3"><ANNOTATION_VALUE>tela</ANNOTATION_VALUE></ALIGNABLE_ANNOTATION></ANNOTATION>
</TIER>
<TIER LINGUISTIC_TYPE_REF="sub" PARENT_REF="tx@A" TIER_ID="words@A">
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="w1" ANNOTATION_REF="u1"><ANNOTATION_VALUE>nikanira</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="w2" ANNOTATION_REF="u1" PREVIOUS_ANNOTATION="w1"><ANNOTATION_VALUE>tela</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="w3" ANNOTATION_REF="u2"><ANNOTATION_VALUE>tela</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
</TIER>
<TIER LINGUISTIC_TYPE_REF="sub" PARENT_REF="words@A" TIER_ID="mb@A">
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="m1" ANNOTATION_REF="w1"><ANNOTATION_VALUE>ni-</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="m2" ANNOTATION_REF="w1" PREVIOUS_ANNOTATION="m1"><ANNOTATION_VALUE>kani</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="m3" ANNOTATION_REF="w1" PREVIOUS_ANNOTATION="m2"><ANNOTATION_VALUE>-ra</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="m4" ANNOTATION_REF="w2"><ANNOTATION_VALUE>tela</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
</TIER>
<TIER LINGUISTIC_TYPE_REF="assoc" PARENT_REF="mb@A" TIER_ID="ps@A">
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="p2" ANNOTATION_REF="m2"><ANNOTATION_VALUE>v</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
</TIER>
<TIER LINGUISTIC_TYPE_REF="assoc" PARENT_REF="mb@A" TIER_ID="ge@A">
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="g1" ANNOTATION_REF="m1"><ANNOTATION_VALUE>1SG-</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="g2" ANNOTATION_REF="m2"><ANNOTATION_VALUE>count</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="g3" ANNOTATION_REF="m3"><ANNOTATION_VALUE>-PST</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="g4" ANNOTATION_REF="m4"><ANNOTATION_VALUE>stone</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
</TIER>
<TIER LINGUISTIC_TYPE_REF="assoc" PARENT_REF="tx@A" TIER_ID="ft@A">
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="f1" ANNOTATION_REF="u1"><ANNOTATION_VALUE>I counted stones</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
</TIER>
<LINGUISTIC_TYPE GRAPHIC_REFERENCES="false" LINGUISTIC_TYPE_ID="utt" TIME_ALIGNABLE="true"/>
<LINGUISTIC_TYPE CONSTRAINTS="Symbolic_Subdivision" GRAPHIC_REFERENCES="false" LINGUISTIC_TYPE_ID="sub" TIME_ALIGNABLE="false"/>
<LINGUISTIC_TYPE CONSTRAINTS="Symbolic_Association" GRAPHIC_REFERENCES="false" LINGUISTIC_TYPE_ID="assoc" TIME_ALIGNABLE="false"/>
<CONSTRAINT DESCRIPTION="" STEREOTYPE="Symbolic_Subdivision"/>
<CONSTRAINT DESCRIPTION="" STEREOTYPE="Symbolic_Association"/>
</ANNOTATION_DOCUMENT>
"#;

fn words_and_morphs(p: &Project, seg: &str) -> Vec<(String, Vec<(String, String, String)>)> {
    p.proj
        .members(seg)
        .unwrap()
        .into_iter()
        .map(|tok| {
            let morphs = p
                .proj
                .members(&tok)
                .unwrap()
                .into_iter()
                .map(|m| {
                    let f = |k: &str| p.proj.field_str(&m, k).unwrap().unwrap_or_default();
                    (f("surface"), f("type"), f("gloss"))
                })
                .collect();
            (p.proj.field_str(&tok, "form").unwrap().unwrap(), morphs)
        })
        .collect()
}

#[test]
fn word_morph_and_gloss_tiers_become_the_glossing() {
    let (root, mut p) = temp_project();
    let text = import_eaf(&mut p, GLOSSED, "gloss").unwrap().text;
    let found = layouts(&p, &text).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].names, ["tx@A", "words@A", "mb@A", "ge@A"], "the gloss tier, not the part-of-speech tier");
    let read = read_glossing(&mut p, &text, &found[0]).unwrap();
    assert_eq!((read.sentences, read.words, read.morphs, read.glossed, read.stray), (2, 3, 4, 4, 0));

    let segs = p.proj.ordered_children("text", &text, "segment").unwrap();
    let tier_name = |s: &str| p.proj.field_str(&p.proj.field_str(s, "tier").unwrap().unwrap(), "name").unwrap().unwrap();
    let live: Vec<String> = segs.iter().map(|s| tier_name(s)).collect();
    assert!(live.iter().all(|t| t == "tx@A" || t == "ft@A"), "the read tiers' segments are gone: {live:?}");
    assert_eq!(found[0].kept, vec![(found[0].kept[0].0.clone(), "ps@A".to_string())]);
    let first = segs.iter().find(|s| p.proj.field_str(s, "content").unwrap().as_deref() == Some("nikanira tela")).unwrap();
    assert_eq!(
        words_and_morphs(&p, first),
        vec![
            ("nikanira".to_string(), vec![("ni".into(), "prefix".into(), "1SG".into()), ("kani".into(), "root".into(), "count".into()), ("ra".into(), "suffix".into(), "PST".into())]),
            ("tela".to_string(), vec![("tela".into(), "root".into(), "stone".into())]),
        ]
    );
    let kani = p.proj.members(&p.proj.members(first).unwrap()[0]).unwrap()[1].clone();
    assert_eq!(p.proj.field_str(&kani, "x.elan.ps@A").unwrap().as_deref(), Some("v"), "the part of speech kept on its morph");
    // The second sentence's word has no morphs in the file: a bare word.
    let second = segs.iter().find(|s| p.proj.field_str(s, "content").unwrap().as_deref() == Some("tela")).unwrap();
    assert_eq!(words_and_morphs(&p, second), vec![("tela".to_string(), vec![])]);

    // Out again, the glossing is wugs's own word, morph and gloss tiers; the read tiers are not written twice.
    let out = export_eaf(&p, &text).unwrap();
    assert!(out.contains("TIER_ID=\"tx@A words\"") && out.contains("TIER_ID=\"tx@A glosses\""), "{out}");
    assert!(!out.contains("TIER_ID=\"words@A\"") && !out.contains("TIER_ID=\"ge@A\"") && !out.contains("TIER_ID=\"ps@A\""), "{out}");
    assert!(out.contains(">ni-<") && out.contains(">1SG<") && out.contains(">I counted stones<"));
    assert!(layouts(&p, &text).unwrap().is_empty(), "nothing left to read");

    // The same file again brings nothing back.
    let again = import_eaf(&mut p, GLOSSED, "gloss").unwrap();
    assert_eq!((again.segments_created, again.tiers_created), (0, 0));
    assert_eq!(p.proj.ordered_children("text", &text, "segment").unwrap().len(), segs.len());
    fs::remove_dir_all(root).ok();
}

/// Word tiers timed as Time_Subdivision, glossed whole (no morph tier).
#[test]
fn timed_words_find_their_sentence_by_time() {
    let eaf = r#"<?xml version="1.0" encoding="UTF-8"?>
<ANNOTATION_DOCUMENT AUTHOR="" DATE="2026-01-01T00:00:00+00:00" FORMAT="3.0" VERSION="3.0">
<HEADER MEDIA_FILE="" TIME_UNITS="milliseconds"/>
<TIME_ORDER><TIME_SLOT TIME_SLOT_ID="ts1" TIME_VALUE="0"/><TIME_SLOT TIME_SLOT_ID="ts2" TIME_VALUE="600"/><TIME_SLOT TIME_SLOT_ID="ts3" TIME_VALUE="1200"/></TIME_ORDER>
<TIER LINGUISTIC_TYPE_REF="utt" TIER_ID="utterance">
<ANNOTATION><ALIGNABLE_ANNOTATION ANNOTATION_ID="u1" TIME_SLOT_REF1="ts1" TIME_SLOT_REF2="ts3"><ANNOTATION_VALUE>ama tela</ANNOTATION_VALUE></ALIGNABLE_ANNOTATION></ANNOTATION>
</TIER>
<TIER LINGUISTIC_TYPE_REF="tsub" PARENT_REF="utterance" TIER_ID="word">
<ANNOTATION><ALIGNABLE_ANNOTATION ANNOTATION_ID="w2" TIME_SLOT_REF1="ts2" TIME_SLOT_REF2="ts3"><ANNOTATION_VALUE>tela</ANNOTATION_VALUE></ALIGNABLE_ANNOTATION></ANNOTATION>
<ANNOTATION><ALIGNABLE_ANNOTATION ANNOTATION_ID="w1" TIME_SLOT_REF1="ts1" TIME_SLOT_REF2="ts2"><ANNOTATION_VALUE>ama</ANNOTATION_VALUE></ALIGNABLE_ANNOTATION></ANNOTATION>
</TIER>
<TIER LINGUISTIC_TYPE_REF="assoc" PARENT_REF="word" TIER_ID="gloss">
<ANNOTATION><REF_ANNOTATION ANNOTATION_ID="g1" ANNOTATION_REF="w1"><ANNOTATION_VALUE>father</ANNOTATION_VALUE></REF_ANNOTATION></ANNOTATION>
</TIER>
<LINGUISTIC_TYPE GRAPHIC_REFERENCES="false" LINGUISTIC_TYPE_ID="utt" TIME_ALIGNABLE="true"/>
<LINGUISTIC_TYPE CONSTRAINTS="Time_Subdivision" GRAPHIC_REFERENCES="false" LINGUISTIC_TYPE_ID="tsub" TIME_ALIGNABLE="true"/>
<LINGUISTIC_TYPE CONSTRAINTS="Symbolic_Association" GRAPHIC_REFERENCES="false" LINGUISTIC_TYPE_ID="assoc" TIME_ALIGNABLE="false"/>
</ANNOTATION_DOCUMENT>
"#;
    let (root, mut p) = temp_project();
    let text = import_eaf(&mut p, eaf, "timed").unwrap().text;
    let found = layouts(&p, &text).unwrap();
    assert_eq!(found[0].names, ["utterance", "word", "gloss"]);
    assert_eq!(found[0].morphs, None);
    let read = read_glossing(&mut p, &text, &found[0]).unwrap();
    assert_eq!((read.words, read.morphs, read.glossed), (2, 1, 1));
    let seg = &p.proj.ordered_children("text", &text, "segment").unwrap()[0];
    assert_eq!(words_and_morphs(&p, seg), vec![("ama".to_string(), vec![("ama".into(), "root".into(), "father".into())]), ("tela".to_string(), vec![])], "in time order");
    fs::remove_dir_all(root).ok();
}
