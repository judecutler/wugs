//! A CLDF Dictionary into the lexicon (Dictionaria's datasets, or a wugs
//! export: https://cldf.clld.org): `entries.csv` (Headword, Part_Of_Speech,
//! Citation_Form, Homograph), `senses.csv` (Entry_ID, Description as the
//! gloss, `Gloss_<tag>` other languages, Definition, Usage, Semantic_Domains,
//! Example_IDs) and `examples.csv` (Primary_Text, Translated_Text), read from
//! the folder of the picked `*-metadata.json` or `entries.csv`, and written as
//! `import_lexicon_rows` writes a lexicon table: a row per sense.

use std::collections::HashMap;
use std::path::Path;

use wugs_core::LexiconRow;

use crate::lexicon_table::TableImport;
use crate::session::{project, Session};

/// Whether `path` is a CLDF Dictionary to read (its metadata, or an `entries.csv` with `senses.csv` beside it).
pub fn is_cldf(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_lowercase();
    let dir = path.parent().unwrap_or(Path::new("."));
    (name.ends_with("-metadata.json") || name == "entries.csv") && dir.join("entries.csv").is_file() && dir.join("senses.csv").is_file()
}

struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    fn read(path: &Path) -> Result<Table, String> {
        let (text, tabs) = crate::table_text::read_table(path, &[])?;
        let (header, rows, _) = crate::table_text::records(&text, tabs)?;
        Ok(Table { header, rows })
    }
    /// The first column named one of `names`, case aside.
    fn col(&self, names: &[&str]) -> Option<usize> {
        names.iter().find_map(|n| self.header.iter().position(|h| h.trim().eq_ignore_ascii_case(n)))
    }
}

fn cell(r: &[String], i: Option<usize>) -> String {
    i.and_then(|i| r.get(i)).map(|s| s.trim().to_string()).unwrap_or_default()
}

pub fn import(session: &mut Session, path: &Path) -> Result<TableImport, String> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let entries = Table::read(&dir.join("entries.csv"))?;
    let senses = Table::read(&dir.join("senses.csv"))?;
    let examples = if dir.join("examples.csv").is_file() { Some(Table::read(&dir.join("examples.csv"))?) } else { None };
    let (e_id, e_head) = (entries.col(&["ID"]), entries.col(&["Headword", "Form", "Name"]));
    if e_id.is_none() || e_head.is_none() {
        return Err("entries.csv has no ID and Headword columns, so it is not a CLDF Dictionary's entry table".into());
    }
    let (e_pos, e_cit, e_hom) = (entries.col(&["Part_Of_Speech", "POS"]), entries.col(&["Citation_Form"]), entries.col(&["Homograph"]));
    let (s_entry, s_desc) = (senses.col(&["Entry_ID"]), senses.col(&["Description", "Gloss"]));
    if s_entry.is_none() || s_desc.is_none() {
        return Err("senses.csv has no Entry_ID and Description columns, so it is not a CLDF Dictionary's sense table".into());
    }
    let (s_def, s_usage, s_dom, s_ex) = (senses.col(&["Definition"]), senses.col(&["Usage"]), senses.col(&["Semantic_Domains", "Semantic_Domain"]), senses.col(&["Example_IDs"]));
    // Other languages' glosses: Gloss_<tag> columns, as wugs writes them.
    let tagged: Vec<(usize, String)> = senses.header.iter().enumerate().filter_map(|(i, h)| h.trim().strip_prefix("Gloss_").map(|t| (i, t.to_lowercase()))).filter(|(_, t)| !t.is_empty()).collect();
    let mut by_entry: HashMap<String, &Vec<String>> = HashMap::new();
    for r in &entries.rows {
        by_entry.insert(cell(r, e_id), r);
    }
    let mut example_of: HashMap<String, (String, String)> = HashMap::new();
    if let Some(ex) = &examples {
        let (x_id, x_text, x_tr) = (ex.col(&["ID"]), ex.col(&["Primary_Text"]), ex.col(&["Translated_Text"]));
        for r in &ex.rows {
            example_of.insert(cell(r, x_id), (cell(r, x_text), cell(r, x_tr)));
        }
    }
    let (by_code, by_label) = if s_dom.is_some() && senses.rows.iter().any(|r| !cell(r, s_dom).is_empty()) { crate::lexicon_table::domain_maps(session)? } else { Default::default() };
    let mut unmatched = Vec::new();
    let mut rows = Vec::new();
    let mut no_lemma = 0;
    let mut used_entries = std::collections::HashSet::new();
    for r in &senses.rows {
        let Some(e) = by_entry.get(&cell(r, s_entry)) else {
            no_lemma += 1;
            continue;
        };
        used_entries.insert(cell(r, s_entry));
        let lemma = cell(e, e_head);
        if lemma.is_empty() {
            no_lemma += 1;
            continue;
        }
        let first_example = cell(r, s_ex).split_whitespace().find_map(|id| example_of.get(id).cloned()).unwrap_or_default();
        rows.push(LexiconRow {
            lemma,
            homograph: cell(e, e_hom).parse::<u32>().ok().filter(|n| *n > 0),
            category: cell(e, e_pos),
            citation: cell(e, e_cit),
            gloss: cell(r, s_desc),
            glosses: tagged.iter().map(|(i, t)| (t.clone(), cell(r, Some(*i)))).filter(|(_, g)| !g.is_empty()).collect(),
            definition: cell(r, s_def),
            usage: cell(r, s_usage),
            domains: crate::lexicon_table::domains_in(&cell(r, s_dom), &by_code, &by_label, &mut unmatched),
            example: first_example.0,
            example_translation: first_example.1,
        });
    }
    // An entry with no sense still comes in, with one empty sense to gloss.
    for r in &entries.rows {
        if !used_entries.contains(&cell(r, e_id)) && !cell(r, e_head).is_empty() {
            rows.push(LexiconRow { lemma: cell(r, e_head), homograph: cell(r, e_hom).parse::<u32>().ok().filter(|n| *n > 0), category: cell(r, e_pos), citation: cell(r, e_cit), gloss: String::new(), glosses: Vec::new(), definition: String::new(), usage: String::new(), domains: Vec::new(), example: String::new(), example_translation: String::new() });
        }
    }
    let p = project(session)?;
    let (done, _) = p.import_lexicon_rows(&rows)?;
    let mut columns = vec!["entries.csv Headword → lemma".to_string(), "senses.csv Description → gloss".to_string()];
    for (i, what) in [(e_pos, "entries.csv Part_Of_Speech → category"), (e_cit, "entries.csv Citation_Form → citation form"), (e_hom, "entries.csv Homograph → homograph number"), (s_def, "senses.csv Definition → definition"), (s_usage, "senses.csv Usage → usage"), (s_dom, "senses.csv Semantic_Domains → semantic domains"), (s_ex, "examples.csv → example sentence")] {
        if i.is_some() {
            columns.push(what.to_string());
        }
    }
    columns.extend(tagged.iter().map(|(_, t)| format!("senses.csv Gloss_{t} → gloss in {t}")));
    Ok(TableImport {
        entries: done.entries,
        senses: done.senses,
        unchanged: done.unchanged,
        ambiguous: done.ambiguous,
        no_lemma,
        columns,
        ignored: Vec::new(),
        domains: done.domains,
        domains_unmatched: unmatched,
        examples: done.examples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("wugs-cldf-import-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_dictionaria_style_dataset_comes_in_by_its_cldf_columns() {
        let dir = fresh("dict");
        let data = dir.join("cldf");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join("Dictionary-metadata.json"), "{}").unwrap();
        std::fs::write(data.join("entries.csv"), "ID,Language_ID,Headword,Part_Of_Speech,Homograph\ne1,x,kuma,n,\ne2,x,tela,v,1\ne3,x,tela,n,2\ne4,x,nosense,adv,\n").unwrap();
        std::fs::write(data.join("senses.csv"), "ID,Entry_ID,Description,Gloss_fr,Semantic_Domains,Example_IDs\ns1,e1,bear,ours,1.1 Sky; Moonbeams,x1\ns2,e1,strength,,,\ns3,e2,weave,tisser,,\ns4,e3,cloth,,,\ns5,e9,lost,,,\n").unwrap();
        std::fs::write(data.join("examples.csv"), "ID,Language_ID,Primary_Text,Translated_Text\nx1,x,kuma lari,the bear runs\n").unwrap();
        let mut session: Session = None;
        crate::session::create_project(&mut session, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        assert!(is_cldf(&data.join("Dictionary-metadata.json")));
        assert!(is_cldf(&data.join("entries.csv")));
        assert!(!is_cldf(&data.join("senses.csv")));
        let r = crate::lexicon_table::import(&mut session, &data.join("Dictionary-metadata.json")).unwrap();
        assert_eq!((r.entries, r.senses, r.no_lemma, r.examples, r.domains), (4, 5, 1, 1, 1), "{r:?}");
        assert_eq!(r.domains_unmatched, vec!["Moonbeams".to_string()]);
        assert!(r.columns.contains(&"senses.csv Gloss_fr → gloss in fr".to_string()), "{:?}", r.columns);
        let again = crate::lexicon_table::import(&mut session, &data.join("entries.csv")).unwrap();
        assert_eq!((again.entries, again.senses), (0, 0), "{again:?}");
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wugs_cldf_export_comes_back_into_a_new_project() {
        let dir = fresh("round");
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir.join("sample"), "tester").unwrap();
        let before = crate::lexicon::lexicon_list(&mut session, "").unwrap().len();
        let published = crate::interchange::export_cldf(&mut session, &dir, "2026-11-24", false, false, "wugs test").unwrap();
        drop(session);
        let mut session: Session = None;
        crate::session::create_project(&mut session, dir.join("new").to_str().unwrap(), "prs_Ana").unwrap();
        let meta = Path::new(&published.path).join("Dictionary-metadata.json");
        let r = crate::lexicon_table::import(&mut session, &meta).unwrap();
        assert_eq!((r.entries, r.senses), (published.entries, published.senses), "{r:?}");
        assert_eq!(crate::lexicon::lexicon_list(&mut session, "").unwrap().len(), before);
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
