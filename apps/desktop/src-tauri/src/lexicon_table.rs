//! The lexicon as a spreadsheet: many linguists keep their dictionary in
//! Excel or Google Sheets before any dictionary program, and want one back
//! out for checking and sharing. A table comes in a row per sense (rows of
//! one lemma make one entry), through `Project::import_lexicon_rows`, so the
//! same table twice adds nothing; the lexicon goes out the same shape, with
//! headers this import reads back.

use std::path::Path;

use serde::Serialize;
use wugs_core::LexiconRow;

use crate::lexicon::{entry_view, lexicon_list};
use crate::session::{project, Session};

/// A table larger than this is refused; dictionaries are far smaller.
const MAX_BYTES: usize = 20_000_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableImport {
    pub entries: usize,
    pub senses: usize,
    /// Rows already in the lexicon (the entry has a sense with that gloss).
    pub unchanged: usize,
    /// Lemmas several entries share, written with no homograph number.
    pub ambiguous: Vec<String>,
    /// Rows with no lemma.
    pub no_lemma: usize,
    /// How each column was read, in words: "Headword → lemma".
    pub columns: Vec<String>,
    /// Columns left out.
    pub ignored: Vec<String>,
    /// Links of new senses to semantic domains, and domains named in the
    /// table that the project has neither by code nor by name.
    pub domains: usize,
    pub domains_unmatched: Vec<String>,
    /// Example sentences of new senses.
    pub examples: usize,
}

fn name(header: &str) -> String {
    header.trim().to_lowercase().replace(['_', '-'], " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The language tag of a column of glosses in another language: `gloss (fr)`,
/// `gloss:fr`, `gloss fr`, `gloss_fr`.
fn gloss_tag(header: &str) -> Option<String> {
    let h = header.trim().to_lowercase();
    let rest = h.strip_prefix("gloss")?;
    if !rest.starts_with([' ', ':', '_', '-', '(']) {
        return None;
    }
    let tag = rest.trim_start_matches([' ', ':', '_', '-', '(']).trim_end_matches(')').trim();
    let ok = (2..=35).contains(&tag.len()) && tag.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    ok.then(|| tag.to_string())
}

#[derive(Default)]
struct Columns {
    lemma: Option<usize>,
    gloss: Option<usize>,
    category: Option<usize>,
    citation: Option<usize>,
    homograph: Option<usize>,
    definition: Option<usize>,
    usage: Option<usize>,
    domains: Option<usize>,
    example: Option<usize>,
    example_translation: Option<usize>,
    glosses: Vec<(usize, String)>,
}

fn columns(header: &[String]) -> Result<Columns, String> {
    let find = |names: &[&str]| header.iter().position(|h| names.contains(&name(h).as_str()));
    let mut c = Columns {
        lemma: find(&["lemma", "headword", "head word", "lexeme", "word", "form", "entry", "vernacular", "lx", "orthography"]),
        gloss: find(&["gloss", "english", "meaning", "translation", "ge", "english gloss", "gloss en"]),
        category: find(&["category", "pos", "part of speech", "ps", "word class", "class"]),
        citation: find(&["citation", "citation form", "lc"]),
        homograph: find(&["homograph", "hm", "homograph number"]),
        definition: find(&["definition", "def", "de", "dn"]),
        usage: find(&["usage", "usage note", "note", "notes"]),
        domains: find(&["semantic domains", "semantic domain", "domains", "domain", "sd", "semantic field", "semantic fields"]),
        example: find(&["example", "example sentence", "examples", "xv", "example vernacular"]),
        example_translation: find(&["example translation", "translation of example", "example gloss", "xe", "example english", "example free translation"]),
        glosses: Vec::new(),
    };
    for (i, h) in header.iter().enumerate() {
        if Some(i) == c.gloss {
            continue;
        }
        if let Some(tag) = gloss_tag(h) {
            if c.gloss.is_none() {
                // The only glosses are in a named language: they are the glosses.
                c.gloss = Some(i);
            } else {
                c.glosses.push((i, tag));
            }
        }
    }
    if c.lemma.is_none() {
        // Else the first column that is none of the others.
        let taken: Vec<usize> = [c.gloss, c.category, c.citation, c.homograph, c.definition, c.usage, c.domains, c.example, c.example_translation].into_iter().flatten().chain(c.glosses.iter().map(|g| g.0)).collect();
        c.lemma = (0..header.len()).find(|i| !taken.contains(i));
    }
    if c.lemma.is_none() {
        return Err("no column holds the words: name one lemma, headword or word".into());
    }
    Ok(c)
}

/// Read a lexicon table (CSV, tab-separated or Excel's text export).
pub fn import(session: &mut Session, path: &Path) -> Result<TableImport, String> {
    if std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?.len() > MAX_BYTES as u64 {
        return Err("a lexicon table is at most 20 MB".into());
    }
    let (text, tabs) = crate::table_text::read_table(path, &["tsv", "tab", "txt"])?;
    let (header, rows, _) = crate::table_text::records(&text, tabs)?;
    let c = columns(&header)?;
    let cell = |r: &Vec<String>, i: Option<usize>| i.and_then(|i| r.get(i)).map(|s| s.trim().to_string()).unwrap_or_default();
    // Domains by outline code (`1.1`, `1.1 Sky`) or by name; a table naming
    // domains in a project with none loads the SIL list first, as LIFT does.
    let named = c.domains.is_some() && rows.iter().any(|r| !cell(r, c.domains).is_empty());
    let mut by_code: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut by_label: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if named {
        let p = project(session)?;
        crate::lexicon::sil_domains_if_none(p)?;
        for d in crate::lexicon::domains_list(session)? {
            by_label.entry(d.label.trim().to_lowercase()).or_insert_with(|| d.id.clone());
            by_code.insert(d.code.clone(), d.id);
        }
    }
    let mut unmatched: Vec<String> = Vec::new();
    let mut no_lemma = 0;
    let mut table = Vec::new();
    for r in &rows {
        let lemma = cell(r, c.lemma);
        if lemma.is_empty() {
            no_lemma += 1;
            continue;
        }
        table.push(LexiconRow {
            lemma,
            homograph: cell(r, c.homograph).parse::<u32>().ok().filter(|n| *n > 0),
            category: cell(r, c.category),
            citation: cell(r, c.citation),
            gloss: cell(r, c.gloss),
            glosses: c.glosses.iter().map(|(i, tag)| (tag.clone(), cell(r, Some(*i)))).collect(),
            definition: cell(r, c.definition),
            usage: cell(r, c.usage),
            domains: cell(r, c.domains)
                .split([';', '|'])
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .filter_map(|d| {
                    let code = d.split_whitespace().next().filter(|w| w.chars().all(|ch| ch.is_ascii_digit() || ch == '.')).map(|w| w.trim_end_matches('.'));
                    let found = code.and_then(|c| by_code.get(c)).or_else(|| by_label.get(&d.to_lowercase())).cloned();
                    if found.is_none() && !unmatched.iter().any(|u| u == d) {
                        unmatched.push(d.to_string());
                    }
                    found
                })
                .collect(),
            example: cell(r, c.example),
            example_translation: cell(r, c.example_translation),
        });
    }
    let p = project(session)?;
    let (r, _) = p.import_lexicon_rows(&table)?;
    let mut read = vec![(c.lemma, "lemma".to_string()), (c.gloss, "gloss".into()), (c.category, "category".into()), (c.citation, "citation form".into()), (c.homograph, "homograph number".into()), (c.definition, "definition".into()), (c.usage, "usage".into()), (c.domains, "semantic domains".into()), (c.example, "example sentence".into()), (c.example_translation, "example's translation".into())];
    read.extend(c.glosses.iter().map(|(i, tag)| (Some(*i), format!("gloss in {tag}"))));
    let used: Vec<usize> = read.iter().filter_map(|(i, _)| *i).collect();
    Ok(TableImport {
        entries: r.entries,
        senses: r.senses,
        unchanged: r.unchanged,
        ambiguous: r.ambiguous,
        no_lemma,
        columns: read.into_iter().filter_map(|(i, what)| i.map(|i| format!("{} → {what}", header[i].trim()))).collect(),
        ignored: header.iter().enumerate().filter(|(i, h)| !used.contains(i) && !h.trim().is_empty()).map(|(_, h)| h.trim().to_string()).collect(),
        domains: r.domains,
        domains_unmatched: unmatched,
        examples: r.examples,
    })
}

/// The lexicon as a CSV table, a row per sense, written as a new file in
/// `folder` (never inside the project). Excel opens it as UTF-8 (a BOM).
pub fn export(session: &mut Session, folder: &Path) -> Result<String, String> {
    crate::interchange::outside_project(session, folder)?;
    let rows = lexicon_list(session, "")?;
    let p = project(session)?;
    let mut views = Vec::new();
    for r in &rows {
        views.push(entry_view(p, &r.id)?);
    }
    let mut tags: Vec<String> = views.iter().flat_map(|v| v.senses.iter().flat_map(|s| s.glosses_by_lang.keys().cloned())).collect();
    tags.sort();
    tags.dedup();
    let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
    let mut head = vec!["lemma".to_string(), "homograph".into(), "category".into(), "citation".into(), "sense".into(), "gloss".into()];
    head.extend(tags.iter().map(|t| format!("gloss ({t})")));
    head.extend(["definition".into(), "usage".into(), "semantic domains".into(), "example".into(), "example translation".into()]);
    w.write_record(&head).map_err(|e| e.to_string())?;
    for v in &views {
        let senses = if v.senses.is_empty() { vec![None] } else { v.senses.iter().map(Some).collect() };
        for (n, s) in senses.into_iter().enumerate() {
            let mut rec = vec![v.lemma.clone(), v.homograph.map(|h| h.to_string()).unwrap_or_default(), v.pos_of_speech.clone(), v.citation.clone(), (n + 1).to_string()];
            rec.push(s.map(|s| s.gloss.clone()).unwrap_or_default());
            rec.extend(tags.iter().map(|t| s.and_then(|s| s.glosses_by_lang.get(t).cloned()).unwrap_or_default()));
            rec.push(s.map(|s| s.definition.clone()).unwrap_or_default());
            rec.push(s.map(|s| s.usage.clone()).unwrap_or_default());
            rec.push(s.map(|s| s.domains.iter().map(|d| format!("{} {}", d.code, d.label)).collect::<Vec<_>>().join("; ")).unwrap_or_default());
            // The first example; the table has room for one a sense.
            let ex = s.and_then(|s| s.examples.first());
            rec.push(ex.map(|e| e.sentence.clone()).unwrap_or_default());
            rec.push(ex.map(|e| e.translation.clone()).unwrap_or_default());
            w.write_record(&rec).map_err(|e| e.to_string())?;
        }
    }
    let mut bytes = "\u{feff}".as_bytes().to_vec();
    bytes.extend(w.into_inner().map_err(|e| e.to_string())?);
    let path = crate::interchange::write_new(folder, "lexicon", "csv", &bytes)?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_found_by_their_usual_names() {
        let h = |cols: &[&str]| cols.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let c = columns(&h(&["Headword", "Part of speech", "English", "Gloss (fr)", "Notes"])).unwrap();
        assert_eq!((c.lemma, c.category, c.gloss, c.usage), (Some(0), Some(1), Some(2), Some(4)));
        assert_eq!(c.glosses, vec![(3, "fr".to_string())]);
        let c = columns(&h(&["Kesi", "gloss_es"])).unwrap();
        assert_eq!((c.lemma, c.gloss), (Some(0), Some(1)), "a lone named-language gloss column is the gloss; the first other column the lemma");
        assert_eq!(gloss_tag("Gloss: pt-BR").as_deref(), Some("pt-br"));
        assert_eq!(gloss_tag("glossary"), None);
    }

    #[test]
    fn a_lexicon_goes_out_as_a_table_and_comes_back_unchanged() {
        let dir = std::env::temp_dir().join(format!("wugs-lexicon-table-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let out = std::env::temp_dir().join(format!("wugs-lexicon-table-out-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        let table = out.join("mine.csv");
        std::fs::write(&table, "Headword,Part of speech,English,Gloss (fr),Definition,Colour\nkuma,n,bear,ours,a large animal,brown\nkuma,n,strength,force,,\nlari,v,run,courir,,\n,n,orphan,,,\n").unwrap();
        let r = import(&mut session, &table).unwrap();
        assert_eq!((r.entries, r.senses, r.unchanged, r.no_lemma), (2, 3, 0, 1));
        assert_eq!(r.ignored, vec!["Colour".to_string()]);
        assert!(r.columns.contains(&"Gloss (fr) → gloss in fr".to_string()), "{:?}", r.columns);
        let again = import(&mut session, &table).unwrap();
        assert_eq!((again.entries, again.senses, again.unchanged), (0, 0, 3));
        let written = export(&mut session, &out).unwrap();
        let text = std::fs::read_to_string(&written).unwrap();
        assert!(text.starts_with('\u{feff}'));
        assert!(text.contains("kuma,,n,,2,strength,force,,,,,"), "{text}");
        // What went out comes back in as nothing new.
        let back = import(&mut session, Path::new(&written)).unwrap();
        assert_eq!((back.entries, back.senses), (0, 0), "{:?}", back.columns);
        // Semantic domains by code or name (the SIL list loaded for them) and an example sentence.
        let more = out.join("more.csv");
        std::fs::write(&more, "Word,Gloss,Semantic domain,Example,Example translation\nlangit,sky,1.1 Sky; weather; Moonbeams,langit biru,a blue sky\n").unwrap();
        let r = import(&mut session, &more).unwrap();
        assert_eq!((r.senses, r.domains, r.examples), (1, 2, 1), "{:?}", r.columns);
        assert_eq!(r.domains_unmatched, vec!["Moonbeams".to_string()]);
        let written = export(&mut session, &out).unwrap();
        let text = std::fs::read_to_string(&written).unwrap();
        assert!(text.contains("langit,,,,1,sky,,,,1.1 Sky; 1.1.3 Weather,langit biru,a blue sky"), "{text}");
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&out);
    }
}
