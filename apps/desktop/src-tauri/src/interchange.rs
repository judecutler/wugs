//! Import and export in the formats other tools read: ELAN, FLEx
//! (FlexText and LIFT), Praat TextGrid and Toolbox, plus sealing the whole
//! project as an archive package. The engines live in their crates
//! (`wugs-elan`, `wugs-flex`, `wugs-legacy`, `wugs-seal`); this module picks
//! one by file type, names the output file and reports in words.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::session::{self, Session};

/// A format wugs reads or writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Elan,
    FlexText,
    TextGrid,
    Lift,
    Toolbox,
    /// A `.txt` that is not Toolbox or Praat: a written text, a segment per
    /// sentence in, a segment per line out (`plaintext`).
    PlainText,
    /// A `.csv` or `.tsv` with a header row: one text per document column
    /// value, a segment per row, other columns kept (`table_text`). Import only.
    Table { tabs: bool },
}

impl Format {
    pub fn parse(id: &str) -> Result<Format, String> {
        Ok(match id {
            "elan" => Format::Elan,
            "flextext" => Format::FlexText,
            "textgrid" => Format::TextGrid,
            "lift" => Format::Lift,
            "toolbox" => Format::Toolbox,
            "plaintext" => Format::PlainText,
            other => return Err(format!("unknown format {other}")),
        })
    }

    fn extension(self) -> &'static str {
        match self {
            Format::Elan => "eaf",
            Format::FlexText => "flextext",
            Format::TextGrid => "TextGrid",
            Format::Lift => "lift",
            Format::Toolbox | Format::PlainText => "txt",
            Format::Table { tabs: true } => "tsv",
            Format::Table { tabs: false } => "csv",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Format::Elan => "ELAN",
            Format::FlexText => "FLEx interlinear text",
            Format::TextGrid => "Praat TextGrid",
            Format::Lift => "LIFT lexicon",
            Format::Toolbox => "Toolbox lexicon",
            Format::PlainText => "plain text",
            Format::Table { .. } => "table of sentences",
        }
    }
}

/// Which format a file is, from its extension, else from its first line
/// (a Toolbox database starts with `\_sh`).
pub fn detect(path: &Path, bytes: &[u8]) -> Option<Format> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "eaf" => return Some(Format::Elan),
        "flextext" => return Some(Format::FlexText),
        "textgrid" => return Some(Format::TextGrid),
        "lift" => return Some(Format::Lift),
        "db" | "sfm" | "lex" => return Some(Format::Toolbox),
        "csv" => return Some(Format::Table { tabs: false }),
        "tsv" | "tab" => return Some(Format::Table { tabs: true }),
        _ => {}
    }
    let head = sniff(&bytes[..bytes.len().min(4096)]);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    // A Toolbox database starts `\_sh`; a hand-made MDF file often has no
    // header and starts straight with a marker such as `\lx`, and most of
    // its lines are marker lines (a text that opens with `\noindent` is not).
    let is_marker = |l: &str| {
        l.trim_start().strip_prefix('\\').is_some_and(|rest| {
            let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            !name.is_empty() && rest[name.len()..].chars().next().is_none_or(char::is_whitespace)
        })
    };
    let first: Vec<&str> = head.lines().filter(|l| !l.trim().is_empty()).take(10).collect();
    let markers = first.iter().filter(|l| is_marker(l)).count();
    // A long `\de` may run over continuation lines, so three markers also do.
    let marker = first.first().is_some_and(|l| is_marker(l)) && (markers >= 3 || markers * 2 > first.len());
    if head.starts_with("\\_sh") || marker {
        Some(Format::Toolbox)
    } else if head.starts_with("File type = \"ooTextFile\"") {
        Some(Format::TextGrid)
    } else if ext == "txt" && tab_table(head) {
        // Excel's "Unicode text": tab-separated columns saved as .txt.
        Some(Format::Table { tabs: true })
    } else if ext == "txt" {
        Some(Format::PlainText)
    } else {
        None
    }
}

/// Excel's ordinary CSV is Windows-1252, which differs from Latin-1 only in
/// 0x80–0x9F (curly quotes, dashes, …, €): when the bytes were not UTF-8 or
/// UTF-16, those characters are mapped to what Excel meant.
fn windows_1252(text: String, bytes: &[u8]) -> String {
    if std::str::from_utf8(bytes).is_ok() || bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return text;
    }
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    text.chars().map(|c| if ('\u{80}'..='\u{9f}').contains(&c) { HIGH[c as usize - 0x80] } else { c }).collect()
}

/// Whether text looks like a tab-separated table: the first lines all hold
/// the same number of tabs, at least one.
fn tab_table(head: &str) -> bool {
    let lines: Vec<&str> = head.lines().filter(|l| !l.trim().is_empty()).take(5).collect();
    let tabs = lines.first().map_or(0, |l| l.matches('\t').count());
    // The first line must read as a header: short names, none a sentence
    // (a story of sentence-tab-translation lines stays plain text).
    let header = lines.first().is_some_and(|l| l.split('\t').all(|c| {
        let c = c.trim();
        c.chars().count() <= 30 && !c.ends_with(['.', '!', '?', '…', '。'])
    }));
    tabs > 0 && header && lines.len() >= 2 && lines.iter().all(|l| l.matches('\t').count() == tabs)
}

/// The start of a file as text, for sniffing only: lossy, so a cut through
/// a character never refuses the file.
fn sniff(head: &[u8]) -> String {
    let utf16 = |be: bool| {
        let units: Vec<u16> = head[2..].chunks_exact(2).map(|c| if be { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) }).collect();
        String::from_utf16_lossy(&units)
    };
    match head {
        [0xFF, 0xFE, ..] => utf16(false),
        [0xFE, 0xFF, ..] => utf16(true),
        _ => String::from_utf8_lossy(head).into_owned(),
    }
}

/// A text with this title whose transcription segments are exactly these units, if any.
fn same_text(p: &wugs_core::Project, title: &str, units: &[String]) -> Result<Option<String>, String> {
    let mut candidates = Vec::new();
    for t in p.proj.entities_of_kind("text")? {
        if p.proj.field_str(&t, "title")?.as_deref() == Some(title) {
            candidates.push(t);
        }
    }
    same_text_among(p, &candidates, units)
}

/// Which of these texts (already found by title) holds exactly these units.
fn same_text_among(p: &wugs_core::Project, candidates: &[String], units: &[String]) -> Result<Option<String>, String> {
    let (_, transcription) = crate::concordance::corpus_tiers(p)?;
    for t in candidates.iter().cloned() {
        if p.proj.is_tombstoned(&t)? {
            continue;
        }
        let mut held = Vec::new();
        for seg in p.proj.ordered_children("text", &t, "segment")? {
            if transcription.contains(&p.proj.field_str(&seg, "tier")?.unwrap_or_default()) {
                held.push(p.proj.field_str(&seg, "content")?.unwrap_or_default());
            }
        }
        if held == units {
            return Ok(Some(t));
        }
    }
    Ok(None)
}

/// XML formats declare UTF-8 or UTF-16; anything else is a damaged file,
/// refused rather than read as Latin-1 (which would store garbled text).
fn decode_xml(bytes: &[u8], name: &str) -> Result<String, String> {
    let utf16 = |rest: &[u8], be: bool| {
        if rest.len() % 2 != 0 {
            return None;
        }
        let units: Vec<u16> = rest.chunks_exact(2).map(|c| if be { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) }).collect();
        String::from_utf16(&units).ok()
    };
    let text = match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, false),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, true),
        [0xEF, 0xBB, 0xBF, rest @ ..] => std::str::from_utf8(rest).ok().map(str::to_string),
        _ => std::str::from_utf8(bytes).ok().map(str::to_string),
    };
    text.ok_or_else(|| format!("{name} is not valid UTF-8 or UTF-16 text; it may be damaged, so nothing was imported"))
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    /// The format read, in words.
    pub format: String,
    /// The text to show afterwards (the first one for a FlexText with several).
    pub text: Option<String>,
    /// Whether the lexicon changed.
    pub lexicon: bool,
    /// What happened, one sentence each.
    pub lines: Vec<String>,
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn not_carried(map: &std::collections::BTreeMap<String, usize>) -> Option<String> {
    let total: usize = map.values().sum();
    (total > 0).then(|| {
        let parts: Vec<String> = map.iter().map(|(k, n)| format!("{k} ×{n}")).collect();
        format!("Not carried into wugs: {}.", parts.join(", "))
    })
}

/// Import one file into the open project.
pub fn import_file(s: &mut Session, path: &str) -> Result<Imported, String> {
    let name = Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    import_file_named(s, path, &name)
}

/// Import the file at `path` as if it were named `file_name`: the format
/// comes from that name's extension, a new text's title from its stem.
pub fn import_file_named(s: &mut Session, path: &str, file_name: &str) -> Result<Imported, String> {
    let bytes = fs::read(path).map_err(|e| format!("{file_name}: {e}"))?;
    let named = Path::new(file_name);
    let format = detect(named, &bytes).ok_or_else(|| {
        format!("{file_name} is not a file wugs can import (ELAN .eaf, FLEx .flextext or .lift, Praat .TextGrid, Toolbox .db/.sfm, a .csv/.tsv table of sentences, or a plain .txt)")
    })?;
    let stem = named.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Imported".into());
    let input = match format {
        // Plain text is read as strictly as XML: a damaged file is refused, never garbled.
        Format::Elan | Format::FlexText | Format::Lift | Format::PlainText => decode_xml(&bytes, file_name)?,
        // Praat and Toolbox write Latin-1 as well as UTF-8 and UTF-16, and so
        // does Excel's ordinary "CSV" (Windows-1252, read as Latin-1).
        Format::TextGrid | Format::Toolbox => wugs_legacy::decode(&bytes).map_err(|e| e.to_string())?,
        Format::Table { .. } => windows_1252(wugs_legacy::decode(&bytes).map_err(|e| e.to_string())?, &bytes),
    };
    let input = input.trim_start_matches('\u{feff}');
    let p = session::project(s)?;
    let mut lines = Vec::new();
    let (text, lexicon) = match format {
        Format::Elan => {
            let r = wugs_elan::import_eaf(p, input, &stem).map_err(|e| e.to_string())?;
            lines.push(if r.text_created { "A new text was made.".into() } else { "It matched a text already here.".into() });
            lines.push(format!(
                "{} and {} added, {} already here.",
                plural(r.tiers_created, "tier", "tiers"),
                plural(r.segments_created, "segment", "segments"),
                plural(r.segments_skipped, "segment was", "segments were")
            ));
            (Some(r.text), false)
        }
        Format::FlexText => {
            let r = wugs_flex::import_flextext(p, input, &stem).map_err(|e| e.to_string())?;
            lines.push(format!(
                "{} made; {}, {} and {} added.",
                plural(r.texts_created, "text", "texts"),
                plural(r.phrases_created, "phrase", "phrases"),
                plural(r.tokens_created, "word", "words"),
                plural(r.morphs_created, "morpheme", "morphemes")
            ));
            if r.phrases_skipped > 0 {
                lines.push(format!("{} already here.", plural(r.phrases_skipped, "phrase was", "phrases were")));
            }
            if r.senses_linked + r.links_ambiguous + r.links_unmatched > 0 {
                lines.push(format!(
                    "{} linked to the lexicon; {} matched more than one sense and {} matched none.",
                    plural(r.senses_linked, "morpheme", "morphemes"),
                    r.links_ambiguous,
                    r.links_unmatched
                ));
            }
            lines.extend(not_carried(&r.not_carried.0));
            (r.texts.first().cloned(), false)
        }
        Format::TextGrid => {
            let r = wugs_legacy::import_textgrid(p, input, &stem).map_err(|e| e.to_string())?;
            lines.push(if r.text_created { "A new text was made.".into() } else { "It matched a text already here.".into() });
            lines.push(format!(
                "{} and {} added, {} already here.",
                plural(r.tiers_created, "tier", "tiers"),
                plural(r.segments_created, "segment", "segments"),
                plural(r.segments_skipped, "segment was", "segments were")
            ));
            (Some(r.text), false)
        }
        Format::Lift => {
            let r = wugs_flex::import_lift(p, input).map_err(|e| e.to_string())?;
            lines.push(format!(
                "{} and {} added, {} already here.",
                plural(r.entries_created, "entry", "entries"),
                plural(r.senses_created, "sense", "senses"),
                plural(r.entries_skipped, "entry was", "entries were")
            ));
            lines.extend(not_carried(&r.not_carried.0));
            (None, true)
        }
        Format::PlainText => {
            // UTF-16 without its mark reads as valid UTF-8 full of NULs: a damaged file.
            if input.contains('\0') {
                return Err(format!("{file_name} is not valid UTF-8 or UTF-16 text; it may be damaged, so nothing was imported"));
            }
            let (units, cut) = crate::plaintext::segments(input);
            if units.is_empty() {
                return Err(format!("{file_name} has no text in it; nothing was imported"));
            }
            // Titles and segments are stored in NFC; compare in NFC too.
            let stem = p.proj.nfc(&stem);
            let units: Vec<String> = units.iter().map(|u| p.proj.nfc(u)).collect();
            // The same file again (same title, same sentences) adds nothing.
            if let Some(t) = same_text(p, &stem, &units)? {
                lines.push("It matched a text already here; nothing was added.".into());
                (Some(t), false)
            } else {
                let tier = session::utterance_tier(p)?;
                let refs: Vec<&str> = units.iter().map(String::as_str).collect();
                let (text, _) = p.import_written_text(&stem, &tier, &refs)?;
                lines.push(match cut {
                    crate::plaintext::Cut::ByLine => format!("A new text with {}, one per line of the file.", plural(units.len(), "segment", "segments")),
                    crate::plaintext::Cut::BySentence => format!("A new text with {}, the file's paragraphs cut into sentences.", plural(units.len(), "segment", "segments")),
                });
                (Some(text), false)
            }
        }
        Format::Table { tabs } => {
            if input.contains('\0') {
                return Err(format!("{file_name} is not valid UTF-8 or UTF-16 text; it may be damaged, so nothing was imported"));
            }
            let table = crate::table_text::parse(input, tabs).map_err(|e| format!("{file_name}: {e}"))?;
            if table.documents.is_empty() {
                return Err(format!("{file_name} has no text in its «{}» column; nothing was imported", table.text_column));
            }
            let tier = session::utterance_tier(p)?;
            let mut translation_tier: Option<String> = None;
            // Texts already here by title, looked up once rather than per document.
            let mut by_title: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
            for t in p.proj.entities_of_kind("text")? {
                by_title.entry(p.proj.field_str(&t, "title")?.unwrap_or_default()).or_default().push(t);
            }
            let (mut made, mut matched, mut segments, mut translations, mut first) = (0usize, 0usize, 0usize, 0usize, None);
            for (title, rows) in &table.documents {
                let title = p.proj.nfc(title.as_deref().unwrap_or(&stem));
                let rows: Vec<(String, String, Vec<(String, String)>)> = rows
                    .iter()
                    .filter(|r| !r.text.trim().is_empty())
                    .map(|r| (p.proj.nfc(r.text.trim()), p.proj.nfc(r.translation.trim()), r.extras.iter().map(|(n, v)| (n.clone(), p.proj.nfc(v))).collect()))
                    .collect();
                let units: Vec<String> = rows.iter().map(|(t, _, _)| t.clone()).collect();
                let candidates = by_title.get(&title).cloned().unwrap_or_default();
                let text = match same_text_among(p, &candidates, &units)? {
                    Some(t) => {
                        matched += 1;
                        t
                    }
                    None => {
                        let refs: Vec<wugs_core::WrittenRow> = rows.iter().map(|(t, tr, x)| wugs_core::WrittenRow { text: t, translation: tr, extras: x.clone() }).collect();
                        made += 1;
                        segments += refs.len();
                        translations += rows.iter().filter(|(_, tr, _)| !tr.is_empty()).count();
                        // The track is found or made once, and only when a new text carries a translation.
                        if translation_tier.is_none() && rows.iter().any(|(_, tr, _)| !tr.is_empty()) {
                            let column = table.translation_column.as_deref().unwrap_or_default();
                            translation_tier = Some(session::translation_track(p, &tier, crate::table_text::specific_translation(column))?);
                        }
                        let id = p.import_written_rows(&title, &tier, translation_tier.as_deref(), &refs)?.0;
                        by_title.entry(title.clone()).or_default().push(id.clone());
                        id
                    }
                };
                first.get_or_insert(text);
            }
            let sep = match table.delimiter {
                b'\t' => "tabs".to_string(),
                b';' => "semicolons".to_string(),
                b',' => "commas".to_string(),
                d => format!("«{}»", d as char),
            };
            lines.push(format!("{} with {} from the column «{}»{}, cells separated by {sep}.", plural(made, "new text", "new texts"), plural(segments, "segment", "segments"), table.text_column, table.group_column.as_ref().map(|g| format!(", one text per «{g}»")).unwrap_or_default()));
            if let (Some(column), true) = (&table.translation_column, translations > 0) {
                lines.push(format!("{} from the column «{column}», each paired with its sentence on a translation track.", plural(translations, "translation", "translations")));
            }
            if table.ungrouped > 0 {
                lines.push(format!("{} had no «{}» and went into «{stem}».", plural(table.ungrouped, "row", "rows"), table.group_column.clone().unwrap_or_default()));
            }
            if matched > 0 {
                lines.push(format!("{} matched a text already here; nothing was added for them.", plural(matched, "document", "documents")));
            }
            if !table.kept.is_empty() {
                let shown: Vec<String> = table.kept.iter().take(8).map(|k| format!("«{k}»")).collect();
                let more = if table.kept.len() > 8 { format!(" and {} more", table.kept.len() - 8) } else { String::new() };
                lines.push(format!("Stored with each sentence, not yet shown in wugs: {}{more}.", shown.join(", ")));
            }
            (first, false)
        }
        Format::Toolbox => {
            let r = wugs_legacy::import_toolbox(p, input).map_err(|e| e.to_string())?;
            lines.push(format!(
                "{} and {} added, {} already here.",
                plural(r.entries_created, "entry", "entries"),
                plural(r.senses_created, "sense", "senses"),
                plural(r.entries_skipped, "entry was", "entries were")
            ));
            if r.entries_skipped_matched_by_headword > 0 {
                lines.push(format!(
                    "{} share a headword with an entry made in wugs and were not imported; merge them by hand.",
                    plural(r.entries_skipped_matched_by_headword, "record", "records")
                ));
            }
            if r.markers_kept_verbatim.total() > 0 {
                lines.push(format!(
                    "{} kept as written (markers wugs has no field for).",
                    plural(r.markers_kept_verbatim.total(), "line", "lines")
                ));
            }
            (None, true)
        }
    };
    Ok(Imported { format: format.name().into(), text, lexicon, lines })
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    /// Where the file went.
    pub path: String,
    /// What the file leaves out, one sentence each.
    pub notes: Vec<String>,
}

/// A file name from a title: letters, digits, spaces, `-` and `_` kept,
/// anything else (path separators included) replaced, never empty and never
/// a name Windows reserves.
pub fn file_stem(title: &str, fallback: &str) -> String {
    // Only what a file system refuses or reads specially is replaced, so
    // combining diacritics and every script stay as typed.
    let cleaned: String = title
        .chars()
        .map(|c| if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '.') { '_' } else { c })
        .collect();
    let mut stem: String = cleaned.trim().trim_matches('_').chars().take(60).collect();
    stem = stem.trim_end().to_string();
    if stem.is_empty() {
        stem = fallback.to_string();
    }
    let upper = stem.to_uppercase();
    let device = |prefix: &str| {
        upper.strip_prefix(prefix).is_some_and(|rest| rest.chars().count() == 1 && rest.chars().all(|c| c.is_ascii_digit() || "¹²³".contains(c)))
    };
    if ["CON", "PRN", "AUX", "NUL"].contains(&upper.as_str()) || device("COM") || device("LPT") {
        stem.push('_');
    }
    stem
}

/// Write `contents` to a new file `<stem>.<ext>` in `folder`, adding
/// " (2)", " (3)" … rather than replacing a file already there.
pub fn write_new(folder: &Path, stem: &str, ext: &str, contents: &[u8]) -> Result<PathBuf, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    for n in 1..1000 {
        let name = if n == 1 { format!("{stem}.{ext}") } else { format!("{stem} ({n}).{ext}") };
        let path = folder.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                let written = f.write_all(contents).and_then(|_| f.sync_all());
                if let Err(e) = written {
                    drop(f);
                    fs::remove_file(&path).ok();
                    return Err(format!("{}: {e}", path.display()));
                }
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    Err(format!("{}: too many files named {stem}", folder.display()))
}

/// Exports and seals go outside the project folder: inside it, `cache/` is
/// wiped on a rebuild and `blobs/` would be copied into every later seal.
fn outside_project(s: &mut Session, folder: &Path) -> Result<(), String> {
    let root = session::project_root(s)?;
    let (Ok(root), Ok(chosen)) = (root.canonicalize(), folder.canonicalize()) else {
        return Err(format!("{} is not a folder", folder.display()));
    };
    if chosen.starts_with(&root) {
        return Err("That folder is inside the project folder. Choose one outside it, such as Documents or Desktop.".into());
    }
    Ok(())
}

/// Export one text as ELAN, FlexText or TextGrid into `folder`.
pub fn export_text(s: &mut Session, text_id: &str, format: Format, folder: &Path) -> Result<Exported, String> {
    outside_project(s, folder)?;
    let p = session::project(s)?;
    if p.proj.entity_kind(text_id)?.as_deref() != Some("text") || p.proj.is_tombstoned(text_id)? {
        return Err(format!("no text {text_id}"));
    }
    let title = p.proj.field_str(text_id, "title")?.unwrap_or_default();
    let mut notes = Vec::new();
    let contents = match format {
        Format::Table { .. } => return Err("export a text as plain text or ELAN; table export is not built yet".into()),
        Format::Elan => wugs_elan::export_eaf(p, text_id)?,
        Format::FlexText => wugs_flex::export_flextext(p, text_id)?,
        Format::TextGrid => {
            let out = wugs_legacy::export_textgrid(p, text_id)?;
            if out.not_exported.total() > 0 {
                let parts: Vec<String> = out.not_exported.0.iter().map(|(k, n)| format!("{k} ×{n}")).collect();
                notes.push(format!("Praat cannot hold some segments, so they are not in the file: {}.", parts.join(", ")));
            }
            out.textgrid
        }
        Format::PlainText => {
            // One tier, in time order: the utterance tier when it holds this
            // text's segments, else the transcription tier with the most.
            let (_, transcription) = crate::concordance::corpus_tiers(p)?;
            let mut by_tier: Vec<(String, Vec<(Option<f64>, usize, String)>)> = Vec::new();
            for (i, seg) in p.proj.ordered_children("text", text_id, "segment")?.into_iter().enumerate() {
                let tier = p.proj.field_str(&seg, "tier")?.unwrap_or_default();
                if !transcription.contains(&tier) {
                    continue;
                }
                let row = (p.proj.field_num(&seg, "start")?, i, p.proj.field_str(&seg, "content")?.unwrap_or_default());
                match by_tier.iter_mut().find(|(t, _)| *t == tier) {
                    Some((_, rows)) => rows.push(row),
                    None => by_tier.push((tier, vec![row])),
                }
            }
            let utterance = session::find_utterance_tier(p)?;
            let pick = by_tier.iter().position(|(t, _)| Some(t) == utterance.as_ref()).or_else(|| (0..by_tier.len()).max_by_key(|&i| by_tier[i].1.len()));
            let mut rows = pick.map(|i| by_tier[i].1.clone()).unwrap_or_default();
            // Timed segments by start, untimed after them in their own order.
            rows.sort_by(|a, b| match (a.0, b.0) {
                (Some(x), Some(y)) => x.total_cmp(&y).then(a.1.cmp(&b.1)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.1.cmp(&b.1),
            });
            if by_tier.len() > 1 {
                if let Some(i) = pick {
                    let name = p.proj.field_str(&by_tier[i].0, "name")?.unwrap_or_default();
                    notes.push(format!("Plain text holds one track: «{name}» was written; the other transcription tracks were left out."));
                }
            }
            crate::plaintext::export(&rows.into_iter().map(|r| r.2).collect::<Vec<_>>())
        }
        Format::Lift | Format::Toolbox => return Err(format!("{} is a lexicon format", format.name())),
    };
    let path = write_new(folder, &file_stem(&title, "text"), format.extension(), contents.as_bytes())?;
    Ok(Exported { path: path.display().to_string(), notes })
}

/// Export the whole lexicon as LIFT or Toolbox into `folder`.
pub fn export_lexicon(s: &mut Session, format: Format, folder: &Path) -> Result<Exported, String> {
    outside_project(s, folder)?;
    let p = session::project(s)?;
    let contents = match format {
        Format::PlainText => return Err("plain text holds texts, not a lexicon".into()),
        Format::Table { .. } => return Err("a table of sentences holds texts, not a lexicon".into()),
        Format::Lift => wugs_flex::export_lift(p)?,
        Format::Toolbox => wugs_legacy::export_toolbox(p)?,
        _ => return Err(format!("{} is a text format", format.name())),
    };
    let path = write_new(folder, "lexicon", format.extension(), contents.as_bytes())?;
    Ok(Exported { path: path.display().to_string(), notes: Vec::new() })
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Sealed {
    pub path: String,
    pub files: usize,
    pub bytes: u64,
    pub texts: usize,
    pub lexemes: usize,
    pub restricted: bool,
}

/// Seal the project into a new archive package `wugs-sealed-<date>` (then
/// `-2`, `-3` …) inside `folder`. `date` is the person's own calendar day.
pub fn seal_project(s: &mut Session, folder: &Path, date: &str) -> Result<Sealed, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    outside_project(s, folder)?;
    let p = session::project(s)?;
    let date = crate::corrections::date_or_today(date);
    let mut out = folder.join(format!("wugs-sealed-{date}"));
    let mut n = 2;
    while out.symlink_metadata().is_ok() {
        out = folder.join(format!("wugs-sealed-{date}-{n}"));
        n += 1;
    }
    let r = wugs_seal::seal(p, &out, &wugs_seal::SealOptions::default())?;
    Ok(Sealed {
        path: out.display().to_string(),
        files: r.files,
        bytes: r.bytes,
        texts: r.texts,
        lexemes: r.lexemes,
        restricted: r.contains_restricted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_are_safe_file_names() {
        assert_eq!(file_stem("Frog story / take 2", "text"), "Frog story _ take 2");
        assert_eq!(file_stem("../../etc", "text"), "etc");
        assert_eq!(file_stem("", "text"), "text");
        assert_eq!(file_stem("con", "text"), "con_");
        assert_eq!(file_stem("COM0", "text"), "COM0_");
        assert_eq!(file_stem("lpt²", "text"), "lpt²_");
        assert_eq!(file_stem("Component", "text"), "Component");
        assert_eq!(file_stem("Köpek ağacı", "text"), "Köpek ağacı");
        assert_eq!(file_stem("Tɔ\u{300}ndɛ", "text"), "Tɔ\u{300}ndɛ", "combining marks stay");
        assert_eq!(file_stem("notes. ", "text"), "notes", "no trailing dot or space");
    }

    #[test]
    fn xml_must_be_utf8_or_utf16() {
        assert_eq!(decode_xml("<a>ɔ̀</a>".as_bytes(), "x").unwrap(), "<a>ɔ̀</a>");
        let le: Vec<u8> = [0xFF, 0xFE].into_iter().chain("<a/>".encode_utf16().flat_map(|u| u.to_le_bytes())).collect();
        assert_eq!(decode_xml(&le, "x").unwrap(), "<a/>");
        assert!(decode_xml(b"<a>\xE9</a>", "x.eaf").unwrap_err().contains("x.eaf"));
    }

    #[test]
    fn detects_by_extension_then_content() {
        assert_eq!(detect(Path::new("a.EAF"), b""), Some(Format::Elan));
        assert_eq!(detect(Path::new("a.TextGrid"), b""), Some(Format::TextGrid));
        assert_eq!(detect(Path::new("dict.txt"), b"\\_sh v3.0  400  Dictionary\n"), Some(Format::Toolbox));
        assert_eq!(detect(Path::new("x.txt"), b"File type = \"ooTextFile\"\n"), Some(Format::TextGrid));
        assert_eq!(detect(Path::new("notes.txt"), b"hello"), Some(Format::PlainText));
        assert_eq!(detect(Path::new("sheet.txt"), b"text\tspeaker\nhello\tMaria\n"), Some(Format::Table { tabs: true }), "Excel's Unicode text");
        assert_eq!(detect(Path::new("corpus.CSV"), b"a,b\n"), Some(Format::Table { tabs: false }));
        assert_eq!(detect(Path::new("story.txt"), b"Kesi mopa toru.\tThe dog came home.\nToru mopa.\tIt ran.\n"), Some(Format::PlainText), "sentence-tab-translation lines are a story");
        assert_eq!(windows_1252("\u{93}Caf\u{e9}\u{94} \u{85} \u{80}".into(), b"\x93Caf\xe9\x94 \x85 \x80"), "\u{201c}Café\u{201d} … €");
        assert_eq!(detect(Path::new("notes.md"), b"hello"), None);
        assert_eq!(detect(Path::new("dict.txt"), b"\n\\lx kanu\n\\ps n\n"), Some(Format::Toolbox), "MDF with no header");
        assert_eq!(detect(Path::new("dict.txt"), b"\\lx\n"), Some(Format::Toolbox));
        let essay = b"\\noindent The fox came to the well.\nIt looked down.\nThen it went home.\n";
        assert_eq!(detect(Path::new("essay.txt"), essay), Some(Format::PlainText), "a text that opens with a command");
        assert_eq!(detect(Path::new("essay.txt"), b"\\ is a backslash"), Some(Format::PlainText));
        let mut utf16: Vec<u8> = vec![0xFF, 0xFE];
        utf16.extend("\\_sh v3.0".encode_utf16().flat_map(|u| u.to_le_bytes()));
        utf16.extend([0x3D, 0xD8]); // half a surrogate pair, cut at the sniffing limit
        assert_eq!(detect(Path::new("dict.txt"), &utf16), Some(Format::Toolbox));
    }
}
