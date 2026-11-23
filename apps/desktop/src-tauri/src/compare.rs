//! Another language's word list, read for comparison with the project's own
//! elicited answers (the Compare word lists panel): a CSV or tab-separated
//! table with a concept column and a form column, and optionally a language
//! column for several languages in one file. EDICTOR/LingPy's TSV
//! (DOCULECT, CONCEPT, FORM / VALUE / IPA) and a CLDF Wordlist's
//! `forms.csv` read as they are. Nothing is written to the project.

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
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() > 50_000_000 {
        return Err("a word list file is at most 50 MB".into());
    }
    let text = crate::interchange::windows_1252(wugs_legacy::decode(&bytes).map_err(|e| e.to_string())?, &bytes);
    let tabs = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("tsv") || e.eq_ignore_ascii_case("tab") || e.eq_ignore_ascii_case("txt"));
    let (header, rows, _) = crate::table_text::records(&text, tabs)?;
    let name = path.file_stem().map(|s| s.to_string_lossy().trim().to_string()).unwrap_or_default();
    read_rows(&name, &header, &rows)
}

pub(crate) fn read_rows(name: &str, header: &[String], rows: &[Vec<String>]) -> Result<OtherList, String> {
    let concept = find(header, &["concepticon_gloss", "concept", "parameter", "parameter_id", "gloss", "meaning", "english"]).ok_or("No concept column: name one Concept, Concepticon_Gloss, Parameter or Gloss.")?;
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
        let l = read_rows("x", &h, &r).unwrap();
        assert_eq!(l.columns, ["DOCULECT", "CONCEPT", "FORM"]);
        assert_eq!(l.forms[1], OtherForm { language: "Teke".into(), concept: "hand".into(), form: "nima".into() });
        assert_eq!(l.skipped, 1);
        let (h, r) = rows("Concepticon_Gloss\tSegments\nHAND\tl i m a\n");
        let l = read_rows("Kanu words", &h, &r).unwrap();
        assert_eq!(l.forms, [OtherForm { language: "Kanu words".into(), concept: "HAND".into(), form: "lima".into() }]);
        let (h, r) = rows("word\tnote\nx\ty\n");
        assert!(read_rows("x", &h, &r).unwrap_err().contains("No concept column"));
    }
}
