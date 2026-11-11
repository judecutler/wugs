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
}

impl Format {
    pub fn parse(id: &str) -> Result<Format, String> {
        Ok(match id {
            "elan" => Format::Elan,
            "flextext" => Format::FlexText,
            "textgrid" => Format::TextGrid,
            "lift" => Format::Lift,
            "toolbox" => Format::Toolbox,
            other => return Err(format!("unknown format {other}")),
        })
    }

    fn extension(self) -> &'static str {
        match self {
            Format::Elan => "eaf",
            Format::FlexText => "flextext",
            Format::TextGrid => "TextGrid",
            Format::Lift => "lift",
            Format::Toolbox => "txt",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Format::Elan => "ELAN",
            Format::FlexText => "FLEx interlinear text",
            Format::TextGrid => "Praat TextGrid",
            Format::Lift => "LIFT lexicon",
            Format::Toolbox => "Toolbox lexicon",
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
        _ => {}
    }
    let head = wugs_legacy::decode(&bytes[..bytes.len().min(4096)]).ok()?;
    let head = head.trim_start_matches('\u{feff}').trim_start();
    if head.starts_with("\\_sh") {
        Some(Format::Toolbox)
    } else if head.starts_with("File type = \"ooTextFile\"") {
        Some(Format::TextGrid)
    } else {
        None
    }
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
    let path = Path::new(path);
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let format = detect(path, &bytes).ok_or_else(|| {
        format!(
            "{} is not a file wugs can import (ELAN .eaf, FLEx .flextext or .lift, Praat .TextGrid, Toolbox .db/.sfm/.txt)",
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        )
    })?;
    let stem = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Imported".into());
    let input = wugs_legacy::decode(&bytes).map_err(|e| e.to_string())?;
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
    let cleaned: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let mut stem: String = cleaned.trim().trim_matches('_').chars().take(60).collect();
    stem = stem.trim().to_string();
    if stem.is_empty() {
        stem = fallback.to_string();
    }
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
        "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem.to_ascii_uppercase().as_str()) {
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

/// Export one text as ELAN, FlexText or TextGrid into `folder`.
pub fn export_text(s: &mut Session, text_id: &str, format: Format, folder: &Path) -> Result<Exported, String> {
    let p = session::project(s)?;
    if p.proj.entity_kind(text_id)?.as_deref() != Some("text") || p.proj.is_tombstoned(text_id)? {
        return Err(format!("no text {text_id}"));
    }
    let title = p.proj.field_str(text_id, "title")?.unwrap_or_default();
    let mut notes = Vec::new();
    let contents = match format {
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
        Format::Lift | Format::Toolbox => return Err(format!("{} is a lexicon format", format.name())),
    };
    let path = write_new(folder, &file_stem(&title, "text"), format.extension(), contents.as_bytes())?;
    Ok(Exported { path: path.display().to_string(), notes })
}

/// Export the whole lexicon as LIFT or Toolbox into `folder`.
pub fn export_lexicon(s: &mut Session, format: Format, folder: &Path) -> Result<Exported, String> {
    let p = session::project(s)?;
    let contents = match format {
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
        assert_eq!(file_stem("Köpek ağacı", "text"), "Köpek ağacı");
    }

    #[test]
    fn detects_by_extension_then_content() {
        assert_eq!(detect(Path::new("a.EAF"), b""), Some(Format::Elan));
        assert_eq!(detect(Path::new("a.TextGrid"), b""), Some(Format::TextGrid));
        assert_eq!(detect(Path::new("dict.txt"), b"\\_sh v3.0  400  Dictionary\n"), Some(Format::Toolbox));
        assert_eq!(detect(Path::new("x.txt"), b"File type = \"ooTextFile\"\n"), Some(Format::TextGrid));
        assert_eq!(detect(Path::new("notes.txt"), b"hello"), None);
    }
}
