//! Tables of text (Scrolls, SCROLLS.md §1: "Plain text, CSV/TSV, …"): a
//! `.csv` or `.tsv` with a header row, one sentence or utterance per row.
//!
//! - **The text column** is the first header named, in this order, sentence,
//!   utterance, transcription, text, content, form, word, lexeme, headword,
//!   vernacular, orthography or ipa (any case) that is not all numbers or
//!   times; else the column whose cells are longest on average, a gloss or
//!   translation column last.
//! - **Documents:** when a header is named document, doc, text_id, file, title
//!   or source, rows are grouped into one text per value, in the order the
//!   values first appear, titled by the value; else the file is one text.
//! - **The translation column** is the first other header named translation,
//!   free translation, free, translated, english or meaning, else one naming
//!   a translation (`Translation (English)`, not `translation status`), and
//!   not a column of yes/no marks; its cell becomes the row's translation,
//!   a segment on a translation track paired with the row's segment. A
//!   gloss column stays a kept column: it may hold morpheme glosses.
//! - **Times:** columns named start and end (or begin, start time, end
//!   time, tmin, tmax) whose every filled cell is a time (seconds, or
//!   `h:mm:ss.mmm` as subtitles write it) time each row whose two cells hold
//!   one, end after start; the rest stay untimed. A table wugs wrote reads
//!   back with its times.
//! - **Every other column is kept** on its row's segment as `x.csv.<header>`
//!   (the header lower-cased, spaces and punctuation as `-`), so speaker,
//!   date or genre columns stay with their sentences.
//!
//! Parsing is the `csv` crate's (quoted cells, embedded newlines, a BOM);
//! rows shorter than the header are padded, longer ones refused.

/// Names of the text column, most preferred first: sentences, then the
/// vernacular word of a word list.
const TEXT_NAMES: [&str; 12] = ["sentence", "utterance", "transcription", "text", "content", "form", "word", "lexeme", "headword", "vernacular", "orthography", "ipa"];
/// Columns that hold a translation of the text, never the text itself when guessing.
const TRANSLATION_NAMES: [&str; 6] = ["gloss", "translation", "free", "english", "meaning", "translit"];
/// Names of the translation column, most preferred first (`translation…` also matches by prefix).
const TRANSLATION_COLUMN: [&str; 7] = ["translation", "translations", "free translation", "free", "translated", "english", "meaning"];
const GROUP_NAMES: [&str; 6] = ["document", "doc", "text_id", "file", "title", "source"];
const START_NAMES: [&str; 6] = ["start", "begin", "start time", "start_time", "starttime", "tmin"];
const END_NAMES: [&str; 5] = ["end", "end time", "end_time", "endtime", "tmax"];

#[derive(Debug, PartialEq)]
pub struct Row {
    pub text: String,
    /// The translation cell, or empty.
    pub translation: String,
    /// `(x.csv.<header>, value)` for the other columns.
    pub extras: Vec<(String, String)>,
    /// Start and end in milliseconds from the start and end columns.
    pub span: Option<(f64, f64)>,
}

#[derive(Debug, PartialEq)]
pub struct Table {
    /// The header the text came from.
    pub text_column: String,
    /// The header rows were grouped by, if any.
    pub group_column: Option<String>,
    /// The header translations came from, if any.
    pub translation_column: Option<String>,
    /// `(document title or None, rows)` in order.
    pub documents: Vec<(Option<String>, Vec<Row>)>,
    /// Headers kept as `x.csv.*` fields.
    pub kept: Vec<String>,
    /// The separator used: `,` `;` tab or `|`.
    pub delimiter: u8,
    /// Rows with an empty document cell (put in the file's own text).
    pub ungrouped: usize,
}

/// A header as a field name: lower case, runs of anything but letters and
/// digits as one `-`; empty when nothing is left.
fn slug(header: &str) -> String {
    let mut out = String::new();
    for c in header.trim().to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

pub fn field_name(header: &str) -> String {
    format!("x.csv.{}", slug(header))
}

/// The separator of a file: an Excel `sep=X` line if there is one (then
/// skipped), else the most frequent of `,` `;` tab `|` in the header line
/// outside quotes; `tabs` (a `.tsv`) prefers tab.
fn delimiter(input: &str, tabs: bool) -> (u8, &str) {
    let first = input.lines().next().unwrap_or("");
    if let Some(d) = first.trim().strip_prefix("sep=").and_then(|r| r.bytes().next()) {
        let rest = input.split_once('\n').map_or("", |(_, r)| r);
        return (d, rest);
    }
    if tabs && first.contains('\t') {
        return (b'\t', input);
    }
    let mut counts = [(b',', 0usize), (b';', 0), (b'\t', 0), (b'|', 0)];
    let mut quoted = false;
    for c in first.bytes() {
        if c == b'"' {
            quoted = !quoted;
        } else if !quoted {
            if let Some(n) = counts.iter_mut().find(|(d, _)| *d == c) {
                n.1 += 1;
            }
        }
    }
    // The first of equal counts wins, so a tie goes to the comma.
    let top = counts.iter().map(|(_, n)| *n).max().unwrap_or(0);
    let best = counts.iter().find(|(_, n)| *n == top && top > 0).map(|(d, _)| *d);
    (best.unwrap_or(if tabs { b'\t' } else { b',' }), input)
}

/// Whether a tab file quotes its cells: some cell opens with `"`, and every
/// such cell on its line also closes with one.
fn tab_quoted(body: &str) -> bool {
    let opening: Vec<&str> = body.lines().flat_map(|l| l.split('\t')).filter(|c| c.starts_with('"')).collect();
    !opening.is_empty() && opening.iter().all(|c| c.len() > 1 && c.ends_with('"'))
}

/// A header naming one translation among others (`Translation (English)`,
/// `translation_es`), which gets a track of its own name; None for the
/// plain names and for headers about translations rather than holding one
/// (`translation status`, `Translation notes`).
pub fn specific_translation(header: &str) -> Option<&str> {
    let lower = header.trim().to_lowercase();
    let rest = lower.strip_prefix("translation")?;
    let about = ["status", "note", "check", "done", "flag", "comment", "by", "date", "who", "ok"];
    let words: Vec<&str> = rest.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    if words.is_empty() || rest.starts_with(|c: char| c.is_alphanumeric()) || words.iter().any(|w| about.iter().any(|a| w == a || (a.len() >= 4 && w.starts_with(a)))) {
        return None;
    }
    Some(header.trim())
}

/// Whether every non-empty cell is a yes/no mark (`yes`, `no`, `x`, `true`, `✓`).
fn flags(cells: &[&str]) -> bool {
    let non_empty: Vec<String> = cells.iter().filter(|c| !c.is_empty()).map(|c| c.to_lowercase()).collect();
    !non_empty.is_empty() && non_empty.iter().all(|c| matches!(c.as_str(), "yes" | "no" | "y" | "n" | "x" | "true" | "false" | "done" | "✓" | "✔"))
}

/// Whether every non-empty cell is a number or a time (`12`, `0:04.5`, `3.2`).
fn numeric(cells: &[&str]) -> bool {
    let non_empty: Vec<&&str> = cells.iter().filter(|c| !c.is_empty()).collect();
    !non_empty.is_empty() && non_empty.iter().all(|c| c.chars().all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | ':' | ',' | '-' | '+')))
}

/// Spreadsheet files read as tables, by extension.
pub const SPREADSHEETS: [&str; 5] = ["xlsx", "xlsm", "xlsb", "xls", "ods"];
/// The largest table file read.
const MAX_TABLE_FILE: u64 = 50_000_000;
/// Rows read from a spreadsheet's sheet.
const MAX_SHEET_ROWS: usize = 200_000;

/// Whether a path names a spreadsheet (Excel or OpenDocument).
pub fn is_spreadsheet(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| SPREADSHEETS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// A table file's text and whether it is tab-separated: a spreadsheet's
/// first sheet with anything in it, as tab-separated text (each cell's tabs
/// and line breaks as spaces, whole numbers without `.0`), else the file
/// decoded as UTF-8/16 or Windows-1252, tab-separated when its extension is
/// one of `tab_extensions`.
pub fn read_table(path: &std::path::Path, tab_extensions: &[&str]) -> Result<(String, bool), String> {
    let size = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?.len();
    if size > MAX_TABLE_FILE {
        return Err(format!("a table is at most {} MB", MAX_TABLE_FILE / 1_000_000));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if is_spreadsheet(path) {
        return Ok((spreadsheet_text(bytes, &path.display().to_string())?, true));
    }
    let text = crate::interchange::windows_1252(wugs_legacy::decode(&bytes).map_err(|e| e.to_string())?, &bytes);
    let tabs = path.extension().and_then(|e| e.to_str()).is_some_and(|e| tab_extensions.iter().any(|x| e.eq_ignore_ascii_case(x)));
    Ok((text, tabs))
}

/// The first sheet with a filled cell, as tab-separated text; the kind of
/// spreadsheet is told from the bytes, `name` only names it in errors.
pub fn spreadsheet_text(bytes: Vec<u8>, name: &str) -> Result<String, String> {
    use calamine::{open_workbook_auto_from_rs, Data, Reader};
    let mut book = open_workbook_auto_from_rs(std::io::Cursor::new(bytes)).map_err(|e| format!("{name}: not a spreadsheet wugs can read ({e})"))?;
    let cell = |d: &Data| -> String {
        let s = match d {
            Data::Empty => String::new(),
            Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
            other => other.to_string(),
        };
        s.replace(['\t', '\r', '\n'], " ").trim().to_string()
    };
    for sheet in book.sheet_names().to_vec() {
        let Ok(range) = book.worksheet_range(&sheet) else { continue };
        let mut lines: Vec<String> = Vec::new();
        for row in range.rows().take(MAX_SHEET_ROWS) {
            let mut cells: Vec<String> = row.iter().map(cell).collect();
            while cells.last().is_some_and(String::is_empty) {
                cells.pop();
            }
            lines.push(cells.join("\t"));
        }
        if lines.iter().any(|l| !l.is_empty()) {
            // Rows before the first filled one are not the header.
            let first = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);
            return Ok(lines[first..].join("\n") + "\n");
        }
    }
    Err(format!("{name}: no sheet has anything in it"))
}

/// A table's header (trimmed, as written) and its rows, each padded to
/// the header's length, line breaks in a cell as spaces, empty rows left
/// out. The separator is found as `parse` documents it.
pub fn records(input: &str, tabs: bool) -> Result<(Vec<String>, Vec<Vec<String>>, u8), String> {
    let input = input.trim_start_matches('\u{feff}');
    let (delim, body) = delimiter(input, tabs);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        // A tab file is cells as typed unless every cell that opens with a
        // quotation mark is fully quoted (Excel's and pandas' tab export).
        .quoting(delim != b'\t' || tab_quoted(body))
        .from_reader(body.as_bytes());
    let raw: Vec<String> = reader.headers().map_err(|e| format!("the header row cannot be read: {e}"))?.iter().map(|h| h.trim().to_string()).collect();
    if raw.iter().all(String::is_empty) {
        return Err("the file has no header row; the first row must name the columns".into());
    }
    let mut records: Vec<Vec<String>> = Vec::new();
    for (i, r) in reader.records().enumerate() {
        let r = r.map_err(|e| format!("row {} cannot be read: {e}", i + 2))?;
        // Line breaks inside a cell become spaces: a segment is one line.
        let mut cells: Vec<String> = r.iter().map(|c| c.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
        if cells.len() > raw.len() {
            if cells[raw.len()..].iter().any(|c| !c.is_empty()) {
                return Err(format!("row {} has {} cells but the header names {}", i + 2, cells.len(), raw.len()));
            }
            cells.truncate(raw.len());
        }
        cells.resize(raw.len(), String::new());
        if cells.iter().any(|c| !c.is_empty()) {
            records.push(cells);
        }
    }
    Ok((raw, records, delim))
}

pub fn parse(input: &str, tabs: bool) -> Result<Table, String> {
    let (raw, records, delim) = records(input, tabs)?;
    // Repeated or empty names get their own field: `note`, `note-2`, `col3`.
    let mut names: Vec<String> = Vec::new();
    for (i, h) in raw.iter().enumerate() {
        let base = if slug(h).is_empty() { format!("col{}", i + 1) } else { slug(h) };
        let mut name = base.clone();
        let mut n = 2;
        while names.contains(&name) {
            name = format!("{base}-{n}");
            n += 1;
        }
        names.push(name);
    }
    let column = |c: usize| records.iter().map(|r| r[c].as_str()).collect::<Vec<_>>();
    let named = |wanted: &str| raw.iter().position(|h| h.to_lowercase() == wanted);
    let text_at = TEXT_NAMES
        .iter()
        .filter_map(|n| named(n))
        .find(|&c| !numeric(&column(c)))
        .or_else(|| {
            (0..raw.len()).filter(|&c| !numeric(&column(c))).max_by_key(|&c| {
                let total: usize = records.iter().map(|r| r[c].chars().count()).sum();
                // A gloss or translation column is the text's companion, ranked last.
                let header = raw[c].to_lowercase();
                let companion = TRANSLATION_NAMES.contains(&header.as_str()) || TRANSLATION_COLUMN.contains(&header.as_str()) || header.starts_with("translation");
                (!companion, (total * 1000) / records.len().max(1))
            })
        })
        .ok_or_else(|| "no column holds text: every column is numbers or times".to_string())?;
    let group_at = GROUP_NAMES.iter().filter_map(|n| named(n)).find(|&g| g != text_at);
    let is_time = |c: usize| column(c).iter().all(|v| v.trim().is_empty() || crate::subtitles::clock(v).is_some()) && column(c).iter().any(|v| !v.trim().is_empty());
    let start_at = START_NAMES.iter().filter_map(|n| named(n)).find(|&c| c != text_at && is_time(c));
    let end_at = END_NAMES.iter().filter_map(|n| named(n)).find(|&c| c != text_at && is_time(c));
    let (start_at, end_at) = match (start_at, end_at) {
        (Some(a), Some(b)) if Some(a) != group_at && Some(b) != group_at => (Some(a), Some(b)),
        _ => (None, None),
    };
    let translation_at = TRANSLATION_COLUMN
        .iter()
        .filter_map(|n| named(n))
        .chain((0..raw.len()).filter(|&c| specific_translation(&raw[c]).is_some()))
        .find(|&c| c != text_at && Some(c) != group_at && !numeric(&column(c)) && !flags(&column(c)));
    // Excel's trailing columns with no header and nothing in them are dropped.
    let kept: Vec<usize> = (0..raw.len()).filter(|&c| c != text_at && Some(c) != group_at && Some(c) != translation_at && Some(c) != start_at && Some(c) != end_at && !(raw[c].is_empty() && records.iter().all(|r| r[c].is_empty()))).collect();
    let mut documents: Vec<(Option<String>, Vec<Row>)> = Vec::new();
    let mut at: std::collections::HashMap<Option<String>, usize> = std::collections::HashMap::new();
    let mut ungrouped = 0;
    for r in records {
        let span = start_at.zip(end_at).and_then(|(a, b)| Some((crate::subtitles::clock(&r[a])?, crate::subtitles::clock(&r[b])?))).filter(|(a, b)| b > a);
        let row = Row { text: r[text_at].clone(), translation: translation_at.map(|c| r[c].clone()).unwrap_or_default(), extras: kept.iter().map(|&c| (format!("x.csv.{}", names[c]), r[c].clone())).collect(), span };
        let key = group_at.map(|g| r[g].clone()).filter(|v| !v.is_empty());
        if group_at.is_some() && key.is_none() {
            ungrouped += 1;
        }
        let i = *at.entry(key.clone()).or_insert_with(|| {
            documents.push((key, Vec::new()));
            documents.len() - 1
        });
        documents[i].1.push(row);
    }
    documents.retain(|(_, rows)| rows.iter().any(|r| !r.text.is_empty()));
    Ok(Table { text_column: raw[text_at].clone(), group_column: group_at.map(|g| raw[g].clone()), translation_column: translation_at.map(|c| raw[c].clone()), documents, kept: kept.iter().map(|&c| raw[c].clone()).collect(), delimiter: delim, ungrouped })
}

/// One text's sentences as a table a spreadsheet opens: a row per sentence
/// with its transcription, translation, the words cut into morphemes and
/// their glosses Leipzig-style (when any word is glossed), its times in
/// seconds (when any is timed) and every column a table import kept
/// (`x.csv.*`), under their stored names. UTF-8 with a BOM, so Excel reads
/// the IPA. Read back by the table import: `sentence` is the text column,
/// `translation` the translation, the rest kept again.
pub fn export(p: &wugs_core::Project, segments: &[String]) -> Result<String, String> {
    struct Row {
        sentence: String,
        translation: String,
        morphemes: String,
        glosses: String,
        start: Option<f64>,
        end: Option<f64>,
        extras: Vec<(String, String)>,
    }
    let mut rows = Vec::new();
    let mut extra_names: Vec<String> = Vec::new();
    for seg in segments {
        let mut words = Vec::new();
        let mut glossed = false;
        for tok in p.proj.members(seg)? {
            if p.proj.is_tombstoned(&tok)? || p.proj.is_punct_token(&tok) {
                continue;
            }
            let morphs = crate::concordance::morphs_of(p, &tok)?;
            if morphs.is_empty() {
                let form = p.proj.field_str(&tok, "form")?.unwrap_or_default();
                words.push((form, "?".to_string()));
            } else {
                glossed = true;
                let surfaces: Vec<(String, String)> = morphs.iter().map(|m| (crate::concordance::bare(&m.surface).to_string(), m.kind.clone())).collect();
                let glosses: Vec<(String, String)> = morphs.iter().map(|m| (if m.gloss.is_empty() { "?".to_string() } else { m.gloss.clone() }, m.kind.clone())).collect();
                words.push((crate::concordance::joined(&surfaces), crate::concordance::joined(&glosses)));
            }
        }
        let mut extras = Vec::new();
        for (field, value) in p.proj.fields_of(seg)? {
            if let Some(name) = field.strip_prefix("x.csv.") {
                let value = value.as_str().map(str::to_string).unwrap_or_default();
                if !extra_names.iter().any(|n| n == name) {
                    extra_names.push(name.to_string());
                }
                extras.push((name.to_string(), value));
            }
        }
        rows.push(Row {
            sentence: p.proj.field_str(seg, "content")?.unwrap_or_default(),
            translation: crate::interchange::translations_of(p, seg)?.into_iter().next().map(|t| t.1).unwrap_or_default(),
            morphemes: if glossed { words.iter().map(|w| w.0.as_str()).collect::<Vec<_>>().join(" ") } else { String::new() },
            glosses: if glossed { words.iter().map(|w| w.1.as_str()).collect::<Vec<_>>().join(" ") } else { String::new() },
            start: p.proj.field_num(seg, "start")?,
            end: p.proj.field_num(seg, "end")?,
            extras,
        });
    }
    let glossed = rows.iter().any(|r| !r.morphemes.is_empty());
    let timed = rows.iter().any(|r| r.start.is_some());
    let mut head = vec!["sentence".to_string(), "translation".to_string()];
    if glossed {
        head.extend(["morphemes".to_string(), "glosses".to_string()]);
    }
    if timed {
        head.extend(["start".to_string(), "end".to_string()]);
    }
    let taken: Vec<String> = head.clone();
    let extra_names: Vec<String> = extra_names.into_iter().filter(|n| !taken.contains(n)).collect();
    head.extend(extra_names.iter().cloned());
    let secs = |ms: Option<f64>| ms.map(|v| format!("{:.3}", v / 1000.0)).unwrap_or_default();
    let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
    w.write_record(&head).map_err(|e| e.to_string())?;
    for r in &rows {
        let mut rec = vec![r.sentence.clone(), r.translation.clone()];
        if glossed {
            rec.extend([r.morphemes.clone(), r.glosses.clone()]);
        }
        if timed {
            rec.extend([secs(r.start), secs(r.end)]);
        }
        rec.extend(extra_names.iter().map(|n| r.extras.iter().find(|e| &e.0 == n).map(|e| e.1.clone()).unwrap_or_default()));
        w.write_record(&rec).map_err(|e| e.to_string())?;
    }
    let bytes = w.into_inner().map_err(|e| e.to_string())?;
    Ok(format!("\u{feff}{}", String::from_utf8(bytes).map_err(|e| e.to_string())?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spreadsheets_read_as_tab_separated_tables() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tables");
        for name in ["lexicon.xlsx", "lexicon.ods"] {
            let (text, tabs) = read_table(&dir.join(name), &[]).unwrap();
            assert!(tabs, "{name}");
            let (header, rows, _) = records(&text, tabs).unwrap();
            assert_eq!(header, ["Headword", "Gloss", "Category", "Number"], "{name}: the empty sheet and row skipped");
            assert_eq!(rows[0], ["kuri", "dog", "noun", "12"], "{name}: a whole number without .0");
            assert_eq!(rows[1], ["mopa", "sleep deeply", "verb", "3.5"], "{name}: a tab inside a cell");
            assert_eq!(rows[2][1], "house home", "{name}: a line break inside a cell");
            assert_eq!(rows.len(), 3, "{name}");
        }
        assert!(is_spreadsheet(std::path::Path::new("a/B.XLSX")));
        assert!(!is_spreadsheet(std::path::Path::new("b.csv")));
        assert!(read_table(&dir.join("README.md"), &["md"]).unwrap().1, "a text file by its extension");
    }

    #[test]
    fn finds_the_text_and_keeps_the_rest() {
        let t = parse("doc,speaker,Sentence,date\nA,Maria,\"Kesi mopa, toru.\",2024\nA,Jo,Mopa toru.,\nB,Maria,\"Two\nlines\",2025\n", false).unwrap();
        assert_eq!(t.text_column, "Sentence");
        assert_eq!(t.group_column.as_deref(), Some("doc"));
        assert_eq!(t.kept, ["speaker", "date"]);
        assert_eq!(t.documents.len(), 2);
        assert_eq!(t.documents[0].0.as_deref(), Some("A"));
        assert_eq!(t.documents[0].1[0], Row { text: "Kesi mopa, toru.".into(), translation: String::new(), extras: vec![("x.csv.speaker".into(), "Maria".into()), ("x.csv.date".into(), "2024".into())], span: None });
        assert_eq!(t.documents[1].1[0].text, "Two lines");
    }

    #[test]
    fn falls_back_to_the_longest_column_and_one_document() {
        let t = parse("\u{feff}id\tgloss\twords\n1\tdog\tthe dog came home\n2\tfish\tthe fish swam away\n", true).unwrap();
        assert_eq!(t.text_column, "words");
        assert_eq!(t.group_column, None);
        assert_eq!(t.documents.len(), 1);
        assert_eq!(field_name(" Speaker Name (L1) "), "x.csv.speaker-name-l1");
        assert!(parse("a,b\n1,2,3\n", false).unwrap_err().contains("row 2"));
        assert!(parse("", false).is_err());
    }

    #[test]
    fn excel_shapes() {
        // European Excel: semicolons, with or without a sep= line.
        for file in ["Satz;Sprecher\nDer Hund kam.;Maria\n", "sep=;\nSatz;Sprecher\nDer Hund kam.;Maria\n"] {
            let t = parse(file, false).unwrap();
            assert_eq!((t.delimiter, t.text_column.as_str()), (b';', "Satz"));
            assert_eq!(t.documents[0].1[0].text, "Der Hund kam.");
        }
        // Preferred names win over header order, and an ID column called text is not the text.
        assert_eq!(parse("line,text\n1,the dog came\n", false).unwrap().text_column, "text");
        assert_eq!(parse("text,sentence\nT1,The dog came.\nT1,It ran.\n", false).unwrap().text_column, "sentence");
        assert_eq!(parse("start,end,word\n0:01.2,0:02.5,dog\n", false).unwrap().text_column, "word");
        // Repeated and empty headers keep every column.
        let t = parse("text,note,Note,#\nhello,a,b,c\n", false).unwrap();
        let names: Vec<&str> = t.documents[0].1[0].extras.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["x.csv.note", "x.csv.note-2", "x.csv.col4"]);
        // A trailing empty cell is fine; a tab file keeps its quotation marks.
        assert!(parse("text,speaker\nhello,Maria,\n", false).is_ok());
        // Word lists take the vernacular word, never the gloss; ties go to the comma.
        assert_eq!(parse("id,word,gloss\n1,kesi,the dog that barks\n", false).unwrap().text_column, "word");
        assert_eq!(parse("lexeme,gloss,notes\nkesi,dog,a long note about it\n", false).unwrap().text_column, "lexeme");
        assert_eq!(parse("text,note; comment\nhello there,x\n", false).unwrap().delimiter, b',');
        // Excel's quoted tab export.
        let t = parse("text\tnote\n\"She said \"\"hi\"\"\"\t1\n", true).unwrap();
        assert_eq!(t.documents[0].1[0].text, "She said \"hi\"");
        assert!(parse("text,speaker,,\nhello,Maria,,\n", false).unwrap().kept == ["speaker"]);
        assert_eq!(parse("text\tn\n\"Hi,\" she said\t1\n", true).unwrap().documents[0].1[0].text, "\"Hi,\" she said");
        // Rows with no document.
        assert_eq!(parse("doc,text\nA,x\n,y\n", false).unwrap().ungrouped, 1);
    }

    #[test]
    fn a_translation_column_pairs_with_its_row() {
        let t = parse("sentence,translation,speaker\nkesi bara,the dog runs,A\nlo,,B\n", false).unwrap();
        assert_eq!(t.translation_column.as_deref(), Some("translation"));
        assert_eq!(t.kept, ["speaker"], "the translation is not also kept");
        assert_eq!(t.documents[0].1[0].translation, "the dog runs");
        assert_eq!(t.documents[0].1[1].translation, "");
        // By prefix, by other names, never the text column, and a gloss column stays kept.
        assert_eq!(parse("Translation (English),text\nthe dog,kesi\n", false).unwrap().translation_column.as_deref(), Some("Translation (English)"));
        assert_eq!(parse("Translation (English),utterance\nthe dog runs away fast,kesi\n", false).unwrap().text_column, "utterance");
        assert_eq!(parse("text,free\nkesi,dog\n", false).unwrap().translation_column.as_deref(), Some("free"));
        assert_eq!(parse("english,meaning\nthe dog,a dog\n", false).unwrap().text_column, "english");
        assert_eq!(parse("english,meaning\nthe dog,a dog\n", false).unwrap().translation_column.as_deref(), Some("meaning"));
        let t = parse("word,gloss\nkesi,dog\n", false).unwrap();
        assert_eq!((t.translation_column, t.kept), (None, vec!["gloss".to_string()]));
        // Headers about translations, and yes/no columns, stay kept columns.
        assert_eq!(parse("text,translation status,Translation (es)\nkesi,done,perro\n", false).unwrap().translation_column.as_deref(), Some("Translation (es)"));
        assert_eq!(parse("text,translated\nkesi,yes\nlo,no\n", false).unwrap().translation_column, None);
        assert_eq!(specific_translation("Translation (English)"), Some("Translation (English)"));
        assert_eq!(specific_translation("translation_es"), Some("translation_es"));
        for plain in ["translation", "Translation notes", "translations", "translation checked", "translated by"] {
            assert_eq!(specific_translation(plain), None, "{plain}");
        }
        // With no preferred text name the longer column is the text, the translation-named one never.
        assert_eq!(parse("id,translation,line\n1,a very long translation here,kesi\n", false).unwrap().text_column, "line");
    }
}
