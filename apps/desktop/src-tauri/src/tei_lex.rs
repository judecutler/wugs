//! The lexicon as a TEI Lex-0 dictionary (https://dariah-eric.github.io/lexicalresources/pages/TEILex0/TEILex0.html),
//! the TEI subset European dictionary portals (ELEXIS, the DARIAH lexical
//! resources) take: an `<entry>` per live entry with its lemma, homograph
//! number, pronunciations, part of speech, etymology and senses; each sense
//! with its definition, its glosses as translation equivalents per language,
//! usage, semantic domains and example sentences with their translations, and
//! relations as cross-references to the other entry. Export only.

use std::collections::HashMap;

use crate::lexicon::{entry_view, EntryView};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// An `xml:id` from a wugs id: letters, digits, `-`, `_`, `.`, starting with a letter.
fn xml_id(prefix: &str, id: &str) -> String {
    let body: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' }).collect();
    format!("{prefix}{body}")
}

/// TEI Lex-0's cross-reference type for a wugs relation.
fn xr_type(relation: &str) -> &str {
    match relation {
        "synonym" => "synonymy",
        "antonym" => "antonymy",
        "compare" => "related",
        "part" | "whole" => "meronymy",
        _ => "related",
    }
}

/// The dictionary, and how many entries and senses it holds.
pub fn export(p: &wugs_core::Project, generator: &str, date: &str) -> Result<(String, usize, usize), String> {
    let lang = p.project_language()?;
    let code = if !lang.iso.trim().is_empty() { lang.iso.trim().to_string() } else if !lang.glottocode.trim().is_empty() { format!("und-x-{}", lang.glottocode.trim()) } else { "und".to_string() };
    let title = if lang.name.trim().is_empty() { "Dictionary".to_string() } else { format!("{} dictionary", lang.name.trim()) };
    let mut entries: Vec<EntryView> = p.proj.entities_of_kind("lexeme")?.iter().filter(|id| p.proj.is_tombstoned(id).map(|d| !d).unwrap_or(false)).map(|id| entry_view(p, id)).collect::<Result<_, _>>()?;
    entries.retain(|e| !e.lemma.trim().is_empty());
    entries.sort_by(|a, b| wugs_model::fold_key(&p.proj.nfc(&a.lemma)).cmp(&wugs_model::fold_key(&p.proj.nfc(&b.lemma))).then(a.homograph.cmp(&b.homograph)).then(a.id.cmp(&b.id)));
    let ids: HashMap<&str, String> = entries.iter().map(|e| (e.id.as_str(), xml_id("e-", &e.id))).collect();
    let mut body = String::new();
    let mut senses = 0;
    for e in &entries {
        let n = e.homograph.map(|h| format!(" n=\"{h}\"")).unwrap_or_default();
        body.push_str(&format!("      <entry xml:id=\"{}\" xml:lang=\"{}\"{n}>\n", ids[e.id.as_str()], esc(&code)));
        body.push_str(&format!("        <form type=\"lemma\">\n          <orth>{}</orth>\n", esc(e.lemma.trim())));
        for pr in e.pronunciations.iter().filter(|x| !x.form.trim().is_empty()) {
            body.push_str(&format!("          <pron>{}</pron>\n", esc(pr.form.trim())));
        }
        body.push_str("        </form>\n");
        if !e.citation.trim().is_empty() && e.citation.trim() != e.lemma.trim() {
            body.push_str(&format!("        <form type=\"variant\">\n          <orth>{}</orth>\n        </form>\n", esc(e.citation.trim())));
        }
        for (_, a) in &e.allomorphs {
            if !a.trim().is_empty() {
                body.push_str(&format!("        <form type=\"variant\">\n          <orth>{}</orth>\n        </form>\n", esc(a.trim())));
            }
        }
        if !e.pos_of_speech.trim().is_empty() {
            body.push_str(&format!("        <gramGrp>\n          <gram type=\"pos\">{}</gram>\n        </gramGrp>\n", esc(e.pos_of_speech.trim())));
        }
        if !e.borrowed_from.trim().is_empty() || !e.etymology.trim().is_empty() {
            body.push_str("        <etym>\n");
            if !e.borrowed_from.trim().is_empty() {
                body.push_str(&format!("          <lang>{}</lang>\n", esc(e.borrowed_from.trim())));
            }
            if !e.etymology.trim().is_empty() {
                body.push_str(&format!("          <seg>{}</seg>\n", esc(e.etymology.trim())));
            }
            body.push_str("        </etym>\n");
        }
        for (i, s) in e.senses.iter().enumerate() {
            senses += 1;
            body.push_str(&format!("        <sense xml:id=\"{}\" n=\"{}\">\n", xml_id("s-", &s.id), i + 1));
            if !s.definition.trim().is_empty() {
                body.push_str(&format!("          <def>{}</def>\n", esc(s.definition.trim())));
            }
            if !s.gloss.trim().is_empty() {
                body.push_str(&format!("          <cit type=\"translationEquivalent\" xml:lang=\"en\">\n            <quote>{}</quote>\n          </cit>\n", esc(s.gloss.trim())));
            }
            for (tag, g) in &s.glosses_by_lang {
                if !g.trim().is_empty() {
                    body.push_str(&format!("          <cit type=\"translationEquivalent\" xml:lang=\"{}\">\n            <quote>{}</quote>\n          </cit>\n", esc(tag), esc(g.trim())));
                }
            }
            if !s.usage.trim().is_empty() {
                body.push_str(&format!("          <usg type=\"hint\">{}</usg>\n", esc(s.usage.trim())));
            }
            for d in &s.domains {
                body.push_str(&format!("          <usg type=\"domain\">{}</usg>\n", esc(format!("{} {}", d.code, d.label).trim())));
            }
            for x in s.examples.iter().filter(|x| !x.sentence.trim().is_empty()) {
                body.push_str(&format!("          <cit type=\"example\">\n            <quote>{}</quote>\n", esc(x.sentence.trim())));
                if !x.translation.trim().is_empty() {
                    body.push_str(&format!("            <cit type=\"translation\" xml:lang=\"en\">\n              <quote>{}</quote>\n            </cit>\n", esc(x.translation.trim())));
                }
                body.push_str("          </cit>\n");
            }
            body.push_str("        </sense>\n");
        }
        for r in &e.relations {
            let Some(target) = ids.get(r.other_id.as_str()) else { continue };
            let label = format!("{}{}", r.other_lemma, r.other_homograph.map(|h| h.to_string()).unwrap_or_default());
            body.push_str(&format!("        <xr type=\"{}\">\n          <lbl>{}</lbl>\n          <ref type=\"entry\" target=\"#{target}\">{}</ref>\n        </xr>\n", xr_type(&r.relation), esc(&r.relation), esc(&label)));
        }
        body.push_str("      </entry>\n");
    }
    let name = if lang.name.trim().is_empty() { "the language" } else { lang.name.trim() };
    let doc = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<TEI xmlns=\"http://www.tei-c.org/ns/1.0\" type=\"lex-0\">\n  <teiHeader>\n    <fileDesc>\n      <titleStmt>\n        <title>{}</title>\n      </titleStmt>\n      <publicationStmt>\n        <publisher>{}</publisher>\n        <availability>\n          <p>Exported on {}. Say here who may use it and under which licence.</p>\n        </availability>\n      </publicationStmt>\n      <sourceDesc>\n        <listBibl type=\"dictionaries\">\n          <bibl>The lexicon of a documentation project of {}, exported by {}.</bibl>\n        </listBibl>\n      </sourceDesc>\n    </fileDesc>\n    <profileDesc>\n      <langUsage>\n        <language role=\"objectLanguage\" ident=\"{}\">{}</language>\n        <language role=\"workingLanguage\" ident=\"en\">English</language>\n      </langUsage>\n    </profileDesc>\n  </teiHeader>\n  <text>\n    <body>\n{body}    </body>\n  </text>\n</TEI>\n",
        esc(&title),
        esc(generator),
        esc(date),
        esc(name),
        esc(generator),
        esc(&code),
        esc(name),
    );
    Ok((doc, entries.len(), senses))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tutorial_lexicon_is_a_well_formed_tei_lex0_dictionary() {
        let dir = std::env::temp_dir().join(format!("wugs-tei-lex-{}", wugs_model::ulid()));
        let mut s: crate::session::Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        let p = crate::session::project(&mut s).unwrap();
        p.set_project_language("Tutorial English", "", "eng").unwrap();
        // An entry with every part TEI Lex-0 carries.
        let ((river, sense), _) = p.add_lexeme("tela", "river").unwrap();
        let ((wade, _), _) = p.add_lexeme("tela", "wade across").unwrap();
        p.set_homograph(&river, Some(1)).unwrap();
        p.set_homograph(&wade, Some(2)).unwrap();
        p.set_lexeme_field(&river, "pos-of-speech", "n").unwrap();
        p.set_lexeme_field(&river, "citation", "telá").unwrap();
        p.set_lexeme_field(&river, "borrowed-from", "Malay").unwrap();
        p.set_lexeme_text(&river, "etymology", "from Malay telaga 'well'").unwrap();
        p.add_allomorph(&river, "tel").unwrap();
        p.add_pronunciation(&river, "ˈtela", None).unwrap();
        p.set_sense_gloss(&sense, "rivière", Some("fr")).unwrap();
        p.set_sense_text(&sense, "definition", "a large stream of water").unwrap();
        p.set_sense_text(&sense, "usage", "also of a canal").unwrap();
        crate::lexicon::sil_domains_if_none(p).unwrap();
        let water = p.proj.entities_of_kind("domain").unwrap().into_iter().find(|d| p.proj.field_str(d, "code").unwrap().as_deref() == Some("1.3")).unwrap();
        p.assign_domain(&sense, &water).unwrap();
        p.add_example(&sense, "tela pana", "the river is big", None, None).unwrap();
        p.add_relation(&river, "compare", &wade).unwrap();
        let (xml, entries, senses) = export(p, "wugs test", "2026-11-24").unwrap();
        assert!(entries >= 5 && senses >= entries, "{entries} {senses}");
        // WUGS_KEEP_TEILEX=<path> keeps the file, to check it against the TEI Lex-0 schema.
        if let Ok(keep) = std::env::var("WUGS_KEEP_TEILEX") {
            std::fs::write(keep, &xml).unwrap();
        }
        let doc = wugs_xml::parse(&xml).unwrap();
        assert_eq!(doc.name, "TEI");
        assert!(xml.contains("<entry xml:id=\"e-") && xml.contains("xml:lang=\"eng\""));
        assert!(xml.contains("<orth>wʌn</orth>"), "{xml}");
        assert!(xml.contains("<cit type=\"translationEquivalent\" xml:lang=\"en\">\n            <quote>one</quote>"), "{xml}");
        assert!(xml.contains("<title>Tutorial English dictionary</title>"));
        for part in ["n=\"1\"", "<pron>ˈtela</pron>", "<form type=\"variant\">\n          <orth>telá</orth>", "<gram type=\"pos\">n</gram>", "<lang>Malay</lang>", "<def>a large stream of water</def>", "xml:lang=\"fr\">\n            <quote>rivière</quote>", "<usg type=\"domain\">1.3 Water</usg>", "<cit type=\"example\">\n            <quote>tela pana</quote>", "<xr type=\"related\">"] {
            assert!(xml.contains(part), "{part} in {xml}");
        }
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
