//! A dictionary to print or share (roadmap 43, the publishing layer): the
//! lexicon as one self-contained HTML document, the format every browser
//! prints to PDF and Word and Google Docs open. Entries in the alphabet's
//! own order (the writing system's `sort`, else case-blind code points),
//! under letter headings; each entry with its homograph number, citation
//! form, part of speech and numbered senses (gloss, other glosses,
//! definition, usage, semantic domains) and, for each sense, one example
//! from the texts with its translation; then a reversal index from glosses
//! to headwords. Examples from texts that are not open are left out unless
//! the person includes them.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use wugs_core::Project;

use crate::lexicon::{entry_view, EntryView};
use crate::session::{self, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dictionary {
    pub path: String,
    pub entries: usize,
    pub senses: usize,
    pub examples: usize,
    /// Examples left out because their text is not open.
    pub restricted_left_out: usize,
    /// Each entry in the page's order, for a site's search.
    #[serde(skip)]
    pub headwords: Vec<Headword>,
}

/// An entry as a site's search lists it: its anchor, headword, homograph number and glosses.
#[derive(Debug, Clone)]
pub struct Headword {
    pub id: String,
    pub lemma: String,
    pub homograph: Option<u32>,
    pub glosses: String,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The project's alphabet: the first writing system that has one.
fn alphabet(p: &Project) -> Result<Vec<Vec<String>>, String> {
    for ws in p.proj.entities_of_kind("ws")? {
        if let Some(sort) = p.proj.field_str(&ws, "sort")? {
            let units = wugs_model::sortkey::sort_units(&sort);
            if !units.is_empty() {
                return Ok(units);
            }
        }
    }
    Ok(Vec::new())
}

/// The letter a headword files under: the alphabet unit it starts with
/// (longest variant first), else its first letter in capitals.
fn letter(units: &[Vec<String>], headword: &str) -> String {
    let word = headword.trim_start_matches(|c: char| wugs_model::sortkey::is_separator(c));
    let mut best: Option<(usize, &Vec<String>)> = None;
    for unit in units {
        for v in unit {
            if !v.is_empty() && word.starts_with(v.as_str()) && best.is_none_or(|(n, _)| v.len() > n) {
                best = Some((v.len(), unit));
            }
        }
    }
    match best {
        Some((_, unit)) => unit.last().cloned().unwrap_or_default(),
        None => word.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default(),
    }
}

/// One example of a sense: the first visible occurrence in a text the
/// export may show, with its first translation.
struct Example {
    text: String,
    translation: Option<String>,
}

fn example(p: &Project, sense: &str, include_restricted: bool, left_out: &mut usize) -> Result<Option<Example>, String> {
    let live = |id: &str| -> Result<bool, String> { Ok(p.proj.entity_kind(id)?.is_some() && !p.proj.is_tombstoned(id)?) };
    let mut morphs = p.proj.children_by_ref("sense", sense, "morph")?;
    morphs.sort();
    let mut skipped = false;
    for morph in morphs {
        let Some(token) = p.proj.field_str(&morph, "token")? else { continue };
        let Some(segment) = p.proj.field_str(&token, "segment")? else { continue };
        let Some(text) = p.proj.field_str(&segment, "text")? else { continue };
        if !(live(&token)? && live(&segment)? && live(&text)?) || !p.proj.members(&token)?.contains(&morph) || !p.proj.members(&segment)?.contains(&token) {
            continue;
        }
        if !include_restricted && p.text_access(&text)? != "open" {
            skipped = true;
            continue;
        }
        let content = p.proj.field_str(&segment, "content")?.unwrap_or_default();
        if content.trim().is_empty() {
            continue;
        }
        let translation = crate::interchange::translations_of(p, &segment)?.into_iter().map(|(_, t)| t).next();
        return Ok(Some(Example { text: content, translation }));
    }
    if skipped {
        *left_out += 1;
    }
    Ok(None)
}

const STYLE: &str = "
@page { size: A4; margin: 18mm 16mm; }
body { font-family: 'Charis SIL', 'Gentium Plus', 'Doulos SIL', 'Noto Serif', Georgia, serif; font-size: 10.5pt; line-height: 1.35; color: #111; max-width: 60em; margin: 2em auto; padding: 0 1em; }
h1 { font-size: 22pt; margin: 0 0 .2em; } .front { margin-bottom: 2em; color: #444; }
.entries, .reversal { column-count: 2; column-gap: 2em; }
h2.letter { column-span: all; font-size: 16pt; border-bottom: 1px solid #999; margin: 1.2em 0 .5em; break-after: avoid; }
.entry { margin: 0 0 .6em; break-inside: avoid; text-indent: -1.2em; padding-left: 1.2em; }
.hw { font-weight: bold; } .hn { font-size: 70%; vertical-align: sub; font-weight: normal; }
.ps { font-style: italic; } .sn { font-weight: bold; } .def, .usage { } .dom { color: #555; font-size: 90%; }
.gl2 { color: #333; } .ex { font-style: italic; } .tr { }
.reversal p { margin: 0 0 .25em; } h2 { break-after: avoid; }
.entry:target { background: #fff3c4; }
@media print { body { margin: 0; max-width: none; } a { color: inherit; text-decoration: none; } }
";

fn entry_html(e: &EntryView, examples: &BTreeMap<String, Vec<Example>>) -> String {
    let mut out = format!("<p class=\"entry\" id=\"{}\"><span class=\"hw\">{}</span>", esc(&e.id), esc(&e.lemma));
    if let Some(n) = e.homograph {
        out.push_str(&format!("<span class=\"hn\">{n}</span>"));
    }
    if !e.citation.is_empty() && e.citation != e.lemma {
        out.push_str(&format!(" ({})", esc(&e.citation)));
    }
    if !e.pos_of_speech.is_empty() {
        out.push_str(&format!(" <span class=\"ps\">{}</span>", esc(&e.pos_of_speech)));
    }
    let numbered = e.senses.len() > 1;
    for (i, s) in e.senses.iter().enumerate() {
        out.push(' ');
        if numbered {
            out.push_str(&format!("<span class=\"sn\">{}</span> ", i + 1));
        }
        out.push_str(&esc(&s.gloss));
        for (tag, g) in &s.glosses_by_lang {
            out.push_str(&format!("; <span class=\"gl2\" lang=\"{}\">{}</span>", esc(tag), esc(g)));
        }
        if !s.definition.is_empty() {
            out.push_str(&format!(". <span class=\"def\">{}</span>", esc(&s.definition)));
        }
        if !s.usage.is_empty() {
            out.push_str(&format!(" <span class=\"usage\">{}</span>", esc(&s.usage)));
        }
        if !s.domains.is_empty() {
            let doms: Vec<String> = s.domains.iter().map(|d| esc(&d.label)).collect();
            out.push_str(&format!(" <span class=\"dom\">[{}]</span>", doms.join(", ")));
        }
        for ex in examples.get(&s.id).into_iter().flatten() {
            out.push_str(&format!(". <span class=\"ex\">{}</span>", esc(ex.text.trim())));
            if let Some(tr) = &ex.translation {
                out.push_str(&format!(" ‘<span class=\"tr\">{}</span>’", esc(tr.trim())));
            }
        }
        out.push('.');
    }
    out.push_str("</p>\n");
    out
}

/// Write the dictionary as `<title>.html` in `folder` (never inside the project, never over a file).
pub fn export_dictionary(s: &mut Session, folder: &Path, date: &str, include_restricted: bool) -> Result<Dictionary, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    crate::interchange::outside_project(s, folder)?;
    let p = session::project(s)?;
    let (title, html, mut d) = document(p, date, include_restricted)?;
    let path = crate::interchange::write_new(folder, &crate::interchange::file_stem(&title, "dictionary"), "html", html.as_bytes())?;
    d.path = path.display().to_string();
    Ok(d)
}

/// The dictionary as one HTML page: its title, the page, and what it holds (`path` empty).
pub(crate) fn document(p: &Project, date: &str, include_restricted: bool) -> Result<(String, String, Dictionary), String> {
    let units = alphabet(p)?;
    let mut entries: Vec<EntryView> = p.proj.entities_of_kind("lexeme")?.iter().map(|id| entry_view(p, id)).collect::<Result<_, _>>()?;
    entries.retain(|e| !e.lemma.trim().is_empty());
    if entries.is_empty() {
        return Err("The lexicon has no entries yet.".into());
    }
    let key = |e: &EntryView| {
        let nfc = p.proj.nfc(&e.lemma);
        if units.is_empty() {
            (wugs_model::fold_key(&nfc).chars().map(|c| c as u32).collect::<Vec<_>>(), nfc, e.homograph)
        } else {
            (wugs_model::sortkey::sort_key(&units, &nfc), nfc, e.homograph)
        }
    };
    entries.sort_by_cached_key(key);
    let mut left_out = 0;
    let mut examples = BTreeMap::new();
    for e in &entries {
        for sense in &e.senses {
            // The examples chosen in the lexicon, else the sense's first use in a text.
            let chosen: Vec<Example> = sense
                .examples
                .iter()
                .map(|x| Example { text: x.sentence.clone(), translation: Some(x.translation.clone()).filter(|t| !t.trim().is_empty()) })
                .collect();
            if !chosen.is_empty() {
                examples.insert(sense.id.clone(), chosen);
            } else if let Some(ex) = example(p, &sense.id, include_restricted, &mut left_out)? {
                examples.insert(sense.id.clone(), vec![ex]);
            }
        }
    }
    let lang = p.project_language()?;
    let title = if lang.name.is_empty() { "Dictionary".to_string() } else { format!("{} dictionary", lang.name) };
    let senses: usize = entries.iter().map(|e| e.senses.len()).sum();
    let mut body = String::new();
    let mut current = String::new();
    body.push_str("<div class=\"entries\">\n");
    for e in &entries {
        let l = letter(&units, &p.proj.nfc(&e.lemma));
        if l != current {
            body.push_str(&format!("<h2 class=\"letter\">{}</h2>\n", esc(&l)));
            current = l;
        }
        body.push_str(&entry_html(e, &examples));
    }
    body.push_str("</div>\n");
    // Reversal index: each primary gloss, case-blind, with the headwords that carry it.
    let mut reversal: BTreeMap<String, (String, Vec<(String, String, Option<u32>)>)> = BTreeMap::new();
    for e in &entries {
        for s in &e.senses {
            let g = s.gloss.trim();
            if g.is_empty() {
                continue;
            }
            let slot = reversal.entry(wugs_model::fold_key(&p.proj.nfc(g))).or_insert_with(|| (g.to_string(), Vec::new()));
            if !slot.1.iter().any(|(id, _, _)| *id == e.id) {
                slot.1.push((e.id.clone(), e.lemma.clone(), e.homograph));
            }
        }
    }
    if !reversal.is_empty() {
        body.push_str("<h2>Index of glosses</h2>\n<div class=\"reversal\">\n");
        for (gloss, words) in reversal.values() {
            let links: Vec<String> = words
                .iter()
                .map(|(id, lemma, hn)| format!("<a href=\"#{}\">{}{}</a>", esc(id), esc(lemma), hn.map(|n| format!("<span class=\"hn\">{n}</span>")).unwrap_or_default()))
                .collect();
            body.push_str(&format!("<p>{} — {}</p>\n", esc(gloss), links.join(", ")));
        }
        body.push_str("</div>\n");
    }
    let lang_attr = if lang.iso.is_empty() { String::new() } else { format!(" lang=\"{}\"", esc(&lang.iso)) };
    let codes: Vec<String> = [(!lang.glottocode.is_empty()).then(|| format!("Glottocode {}", lang.glottocode)), (!lang.iso.is_empty()).then(|| format!("ISO 639-3 {}", lang.iso))].into_iter().flatten().collect();
    let date = crate::corrections::date_or_today(date);
    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>{title}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n\
         <h1{lang_attr}>{title}</h1>\n<p class=\"front\">{entries} entries, {senses} senses{codes}. Made with wugs from the project's lexicon on {date}. \
         A sense's examples are the ones chosen for it in the lexicon, else its first use in the project's texts.{note}</p>\n{body}</body>\n</html>\n",
        title = esc(&title),
        entries = entries.len(),
        codes = if codes.is_empty() { String::new() } else { format!(" · {}", esc(&codes.join(", "))) },
        note = if include_restricted { " Examples from texts that are not open are included: check before sharing." } else { "" },
    );
    let headwords = entries
        .iter()
        .map(|e| Headword {
            id: e.id.clone(),
            lemma: e.lemma.clone(),
            homograph: e.homograph,
            glosses: e.senses.iter().map(|s| s.gloss.trim()).filter(|g| !g.is_empty()).collect::<Vec<_>>().join("; "),
        })
        .collect();
    let d = Dictionary { path: String::new(), entries: entries.len(), senses, examples: examples.values().map(Vec::len).sum(), restricted_left_out: left_out, headwords };
    Ok((title, html, d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_follow_the_alphabet() {
        let units = wugs_model::sortkey::sort_units("a/A b/B ch/Ch c/C");
        assert_eq!(letter(&units, "chata"), "Ch");
        assert_eq!(letter(&units, "-ka"), "K", "a letter outside the alphabet files under itself");
        assert_eq!(letter(&[], "ŋai"), "Ŋ");
    }

    #[test]
    fn the_tutorial_lexicon_prints_with_examples_and_an_index() {
        let dir = std::env::temp_dir().join(format!("wugs-dict-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        session::project(&mut s).unwrap().set_project_language("Tutorial English", "", "eng").unwrap();
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let d = export_dictionary(&mut s, &out, "2026-11-23", false).unwrap();
        assert!(d.path.ends_with("Tutorial English dictionary.html"), "{}", d.path);
        assert!(d.entries >= 5 && d.examples >= 5, "{d:?}");
        let html = std::fs::read_to_string(&d.path).unwrap();
        if let Ok(keep) = std::env::var("WUGS_KEEP_DICTIONARY") {
            std::fs::copy(&d.path, keep).unwrap();
        }
        assert!(html.contains("<span class=\"hw\">wʌn</span>") && html.contains("<span class=\"ex\">wʌn tuː θɹiː</span> ‘<span class=\"tr\">"), "{html}");
        assert!(html.contains("Index of glosses") && html.contains("one — <a href=\"#"), "{html}");
        // The tutorial text is restricted: its examples stay out unless included.
        let text = session::list_texts(&mut s).unwrap()[0].id.clone();
        session::project(&mut s).unwrap().set_access(&text, "restricted").unwrap();
        let closed = export_dictionary(&mut s, &out, "2026-11-23", false).unwrap();
        assert_eq!(closed.examples, 0);
        assert!(closed.restricted_left_out >= 5 && closed.path.ends_with(" (2).html"));
        // An example chosen in the lexicon is printed in place of the first use.
        let p = session::project(&mut s).unwrap();
        let wan = p.proj.entities_of_kind("lexeme").unwrap().into_iter().find(|l| p.proj.field_str(l, "lemma").unwrap().as_deref() == Some("wʌn")).unwrap();
        let sense = p.proj.ordered_children("lexeme", &wan, "sense").unwrap()[0].clone();
        let first = p.proj.children_by_ref("text", &text, "segment").unwrap().into_iter().find(|g| p.proj.field_str(g, "content").unwrap().as_deref() == Some("wʌn tuː θɹiː")).unwrap();
        let refused = crate::lexicon::lexicon_example_from(&mut s, &sense, &first).unwrap_err();
        assert!(refused.contains("not open"), "a restricted sentence is not copied into the lexicon: {refused}");
        crate::lexicon::lexicon_add_example(&mut s, &sense, "wʌn mɔː", "one more").unwrap();
        let chosen = export_dictionary(&mut s, &out, "2026-11-23", false).unwrap();
        let html = std::fs::read_to_string(&chosen.path).unwrap();
        assert!(html.contains("<span class=\"ex\">wʌn mɔː</span> ‘<span class=\"tr\">one more</span>’"), "{html}");
        assert_eq!(chosen.examples, 1);
        // From an open text: the sentence, its translation and where it came from.
        session::project(&mut s).unwrap().set_access(&text, "open").unwrap();
        let e = crate::lexicon::lexicon_example_from(&mut s, &sense, &first).unwrap().entry.unwrap();
        let x = &e.senses[0].examples[1];
        assert_eq!(x.sentence, "wʌn tuː θɹiː");
        assert!(!x.translation.is_empty(), "the sentence's translation comes with it");
        assert_eq!(x.from.as_ref().map(|f| f.segment_id.as_str()), Some(first.as_str()));
        assert!(crate::lexicon::lexicon_example_from(&mut s, &sense, &first).is_err(), "the same sentence twice");
        let e = crate::lexicon::lexicon_set_example(&mut s, &x.id, "translation", "one, two, three").unwrap().entry.unwrap();
        assert_eq!(e.senses[0].examples[1].translation, "one, two, three");
        let e = crate::lexicon::lexicon_delete_example(&mut s, &e.senses[0].examples[0].id).unwrap().entry.unwrap();
        assert_eq!(e.senses[0].examples.len(), 1);
        assert!(export_dictionary(&mut s, &dir.join("sample"), "2026-11-23", false).is_err(), "never inside the project");
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
