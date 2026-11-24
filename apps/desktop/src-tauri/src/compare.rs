//! Another language's word list, read for comparison with the project's own
//! elicited answers (the Compare word lists panel): a CSV or tab-separated
//! table with a concept column and a form column, and optionally a language
//! column for several languages in one file. EDICTOR/LingPy's TSV
//! (DOCULECT, CONCEPT, FORM / VALUE / IPA) and a CLDF Wordlist's
//! `forms.csv` read as they are. Nothing is written to the project.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OtherForm {
    pub language: String,
    pub concept: String,
    pub form: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OtherList {
    /// The file's name, used as the language when it has no language column.
    pub name: String,
    pub forms: Vec<OtherForm>,
    /// The columns read: language (or none), concept, form.
    pub columns: Vec<String>,
    /// Rows with no concept or no form.
    pub skipped: usize,
}

/// Most rows read from one file.
const MAX_ROWS: usize = 200_000;

fn find(header: &[String], names: &[&str]) -> Option<usize> {
    let folded: Vec<String> = header.iter().map(|h| h.trim().to_lowercase().replace([' ', '-'], "_")).collect();
    names.iter().find_map(|n| folded.iter().position(|h| h == n))
}

pub fn read_list(path: &Path) -> Result<OtherList, String> {
    let (text, tabs) = crate::table_text::read_table(path, &["tsv", "tab", "txt"])?;
    let (header, rows, _) = crate::table_text::records(&text, tabs)?;
    let name = path.file_stem().map(|s| s.to_string_lossy().trim().to_string()).unwrap_or_default();
    // A CLDF forms.csv names its concepts and languages by id; the tables
    // beside it say what the ids are.
    let beside = |file: &str, names: &[&str]| path.parent().map(|d| names_of(&d.join(file), names)).unwrap_or_default();
    let names = Names { concepts: beside("parameters.csv", &["concepticon_gloss", "name"]), languages: beside("languages.csv", &["name"]) };
    read_rows(&name, &header, &rows, &names)
}

/// What a CLDF dataset's ids stand for: Parameter_ID to a gloss, Language_ID to a name.
#[derive(Default)]
pub(crate) struct Names {
    concepts: HashMap<String, String>,
    languages: HashMap<String, String>,
}

/// A CLDF table's ID column mapped to the first of `names` it has, filled; empty when the file is missing or unreadable.
fn names_of(path: &Path, names: &[&str]) -> HashMap<String, String> {
    let Ok(bytes) = std::fs::read(path) else { return HashMap::new() };
    if bytes.len() > 50_000_000 {
        return HashMap::new();
    }
    let Ok(text) = wugs_legacy::decode(&bytes) else { return HashMap::new() };
    let Ok((header, rows, _)) = crate::table_text::records(&text, false) else { return HashMap::new() };
    let Some(id) = find(&header, &["id"]) else { return HashMap::new() };
    let columns: Vec<usize> = names.iter().filter_map(|n| find(&header, &[n])).collect();
    rows.iter()
        .filter_map(|r| {
            let value = columns.iter().map(|&i| r.get(i).map(|s| s.trim()).unwrap_or("")).find(|v| !v.is_empty())?;
            Some((r.get(id)?.trim().to_string(), value.to_string()))
        })
        .collect()
}

pub(crate) fn read_rows(name: &str, header: &[String], rows: &[Vec<String>], names: &Names) -> Result<OtherList, String> {
    let concept = if names.concepts.is_empty() { None } else { find(header, &["parameter_id"]) }
        .or_else(|| find(header, &["concepticon_gloss", "concept", "parameter", "parameter_id", "gloss", "meaning", "english"]))
        .ok_or("No concept column: name one Concept, Concepticon_Gloss, Parameter or Gloss.")?;
    let form = find(header, &["form", "ipa", "value", "word", "tokens", "segments", "transcription"]).ok_or("No form column: name one Form, Value, IPA or Word.")?;
    let language = find(header, &["doculect", "language", "language_id", "language_name", "variety", "lect"]);
    let cell = |r: &Vec<String>, i: usize| r.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
    let mut out = OtherList {
        name: name.to_string(),
        forms: Vec::new(),
        columns: [language.map(|i| header[i].clone()), Some(header[concept].clone()), Some(header[form].clone())].into_iter().flatten().collect(),
        skipped: 0,
    };
    for r in rows.iter().take(MAX_ROWS) {
        let (c, mut f) = (cell(r, concept), cell(r, form));
        // A TOKENS or Segments column holds the form cut into sounds; read it joined.
        if matches!(header[form].trim().to_lowercase().as_str(), "tokens" | "segments") {
            f = f.split_whitespace().filter(|t| *t != "+").collect();
        }
        if c.is_empty() || f.is_empty() {
            out.skipped += 1;
            continue;
        }
        let lang = language.map(|i| cell(r, i)).filter(|l| !l.is_empty()).unwrap_or_else(|| name.to_string());
        let lang = names.languages.get(&lang).cloned().unwrap_or(lang);
        let c = names.concepts.get(&c).cloned().unwrap_or(c);
        out.forms.push(OtherForm { language: lang, concept: c, form: f });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(t: &str) -> (Vec<String>, Vec<Vec<String>>) {
        let (h, r, _) = crate::table_text::records(t, true).unwrap();
        (h, r)
    }

    #[test]
    fn edictor_and_plain_tables_read() {
        let (h, r) = rows("ID\tDOCULECT\tCONCEPT\tVALUE\tFORM\tTOKENS\n1\tKanu\thand\tlima\tlima\tl i m a\n2\tTeke\thand\tnima\tnima\tn i m a\n3\tTeke\teye\t\t\t\n");
        let l = read_rows("x", &h, &r, &Names::default()).unwrap();
        assert_eq!(l.columns, ["DOCULECT", "CONCEPT", "FORM"]);
        assert_eq!(l.forms[1], OtherForm { language: "Teke".into(), concept: "hand".into(), form: "nima".into() });
        assert_eq!(l.skipped, 1);
        let (h, r) = rows("Concepticon_Gloss\tSegments\nHAND\tl i m a\n");
        let l = read_rows("Kanu words", &h, &r, &Names::default()).unwrap();
        assert_eq!(l.forms, [OtherForm { language: "Kanu words".into(), concept: "HAND".into(), form: "lima".into() }]);
        let (h, r) = rows("word\tnote\nx\ty\n");
        assert!(read_rows("x", &h, &r, &Names::default()).unwrap_err().contains("No concept column"));
    }

    #[test]
    fn cldf_ids_read_as_glosses_and_names() {
        let dir = std::env::temp_dir().join(format!("wugs-compare-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("forms.csv"), "ID,Language_ID,Parameter_ID,Value,Form,Segments\nk-1,kanu,1_hand,lima,lima,l i m a\nk-2,kanu,2_eye,mata,mata,m a t a\nk-3,teke,9_odd,x,x,x\n").unwrap();
        std::fs::write(dir.join("parameters.csv"), "ID,Name,Concepticon_ID,Concepticon_Gloss\n1_hand,hand,1277,HAND\n2_eye,eye,1248,\n").unwrap();
        std::fs::write(dir.join("languages.csv"), "ID,Name,Glottocode\nkanu,Kanu,kanu1234\n").unwrap();
        let l = read_list(&dir.join("forms.csv")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(l.columns, ["Language_ID", "Parameter_ID", "Form"]);
        let got: Vec<(&str, &str, &str)> = l.forms.iter().map(|f| (f.language.as_str(), f.concept.as_str(), f.form.as_str())).collect();
        // Concepticon gloss first, else the name; an id the tables lack stays as it is.
        assert_eq!(got, [("Kanu", "HAND", "lima"), ("Kanu", "eye", "mata"), ("teke", "9_odd", "x")]);
    }
}
