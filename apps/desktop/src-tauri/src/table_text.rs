//! Tables of text (Scrolls, SCROLLS.md §1: "Plain text, CSV/TSV, …"): a
//! `.csv` or `.tsv` with a header row, one sentence or utterance per row.
//!
//! - **The text column** is the first header named text, sentence, content,
//!   utterance, transcription, line or form (any case); else the column whose
//!   cells are longest on average.
//! - **Documents:** when a header is named document, doc, text_id, file, title
//!   or source, rows are grouped into one text per value, in the order the
//!   values first appear, titled by the value; else the file is one text.
//! - **Every other column is kept** on its row's segment as `x.csv.<header>`
//!   (the header lower-cased, spaces and punctuation as `-`), so speaker,
//!   date or genre columns stay with their sentences.
//!
//! Parsing is the `csv` crate's (quoted cells, embedded newlines, a BOM);
//! rows shorter than the header are padded, longer ones refused.

const TEXT_NAMES: [&str; 7] = ["text", "sentence", "content", "utterance", "transcription", "line", "form"];
const GROUP_NAMES: [&str; 6] = ["document", "doc", "text_id", "file", "title", "source"];

#[derive(Debug, PartialEq)]
pub struct Row {
    pub text: String,
    /// `(x.csv.<header>, value)` for the other columns.
    pub extras: Vec<(String, String)>,
}

#[derive(Debug, PartialEq)]
pub struct Table {
    /// The header the text came from.
    pub text_column: String,
    /// The header rows were grouped by, if any.
    pub group_column: Option<String>,
    /// `(document title or None, rows)` in order.
    pub documents: Vec<(Option<String>, Vec<Row>)>,
    /// Headers kept as `x.csv.*` fields.
    pub kept: Vec<String>,
}

/// A header as a field name: lower case, runs of anything but letters and
/// digits as one `-`.
pub fn field_name(header: &str) -> String {
    let mut out = String::new();
    for c in header.trim().to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    format!("x.csv.{}", out.trim_end_matches('-'))
}

pub fn parse(input: &str, delimiter: u8) -> Result<Table, String> {
    let input = input.trim_start_matches('\u{feff}');
    let mut reader = csv::ReaderBuilder::new().delimiter(delimiter).flexible(true).from_reader(input.as_bytes());
    let headers: Vec<String> = reader.headers().map_err(|e| format!("the header row cannot be read: {e}"))?.iter().map(|h| h.trim().to_string()).collect();
    if headers.iter().all(String::is_empty) {
        return Err("the file has no header row; the first row must name the columns".into());
    }
    let mut records: Vec<Vec<String>> = Vec::new();
    for (i, r) in reader.records().enumerate() {
        let r = r.map_err(|e| format!("row {} cannot be read: {e}", i + 2))?;
        if r.len() > headers.len() {
            return Err(format!("row {} has {} cells but the header names {}", i + 2, r.len(), headers.len()));
        }
        let mut cells: Vec<String> = r.iter().map(|c| c.trim().to_string()).collect();
        cells.resize(headers.len(), String::new());
        if cells.iter().any(|c| !c.is_empty()) {
            records.push(cells);
        }
    }
    let named = |names: &[&str]| headers.iter().position(|h| names.contains(&h.to_lowercase().as_str()));
    let text_at = match named(&TEXT_NAMES) {
        Some(i) => i,
        None => (0..headers.len())
            .max_by_key(|&c| {
                let total: usize = records.iter().map(|r| r[c].chars().count()).sum();
                (total * 1000) / records.len().max(1)
            })
            .unwrap_or(0),
    };
    let group_at = named(&GROUP_NAMES).filter(|&g| g != text_at);
    let kept: Vec<usize> = (0..headers.len()).filter(|&c| c != text_at && Some(c) != group_at && !headers[c].is_empty()).collect();
    let mut documents: Vec<(Option<String>, Vec<Row>)> = Vec::new();
    for r in records {
        let row = Row { text: r[text_at].clone(), extras: kept.iter().map(|&c| (field_name(&headers[c]), r[c].clone())).collect() };
        let key = group_at.map(|g| r[g].clone()).filter(|v| !v.is_empty());
        match documents.iter_mut().find(|(k, _)| *k == key) {
            Some((_, rows)) => rows.push(row),
            None => documents.push((key, vec![row])),
        }
    }
    documents.retain(|(_, rows)| rows.iter().any(|r| !r.text.is_empty()));
    Ok(Table { text_column: headers[text_at].clone(), group_column: group_at.map(|g| headers[g].clone()), documents, kept: kept.iter().map(|&c| headers[c].clone()).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_text_and_keeps_the_rest() {
        let t = parse("doc,speaker,Sentence,date\nA,Maria,\"Kesi mopa, toru.\",2024\nA,Jo,Mopa toru.,\nB,Maria,\"Two\nlines\",2025\n", b',').unwrap();
        assert_eq!(t.text_column, "Sentence");
        assert_eq!(t.group_column.as_deref(), Some("doc"));
        assert_eq!(t.kept, ["speaker", "date"]);
        assert_eq!(t.documents.len(), 2);
        assert_eq!(t.documents[0].0.as_deref(), Some("A"));
        assert_eq!(t.documents[0].1[0], Row { text: "Kesi mopa, toru.".into(), extras: vec![("x.csv.speaker".into(), "Maria".into()), ("x.csv.date".into(), "2024".into())] });
        assert_eq!(t.documents[1].1[0].text, "Two\nlines");
    }

    #[test]
    fn falls_back_to_the_longest_column_and_one_document() {
        let t = parse("\u{feff}id\tgloss\twords\n1\tdog\tthe dog came home\n2\tfish\tthe fish swam away\n", b'\t').unwrap();
        assert_eq!(t.text_column, "words");
        assert_eq!(t.group_column, None);
        assert_eq!(t.documents.len(), 1);
        assert_eq!(field_name(" Speaker Name (L1) "), "x.csv.speaker-name-l1");
        assert!(parse("a,b\n1,2,3\n", b',').unwrap_err().contains("row 2"));
        assert!(parse("", b',').is_err());
    }
}
