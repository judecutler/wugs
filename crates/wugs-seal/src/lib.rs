//! wugs-seal: "Seal a version" (DECISIONS.md, Archival). Writes a BagIt 1.0
//! deposit package for ELAR / PARADISEC / AILLA from a live project without
//! touching it: the project is only read, and the bag is staged beside `out`
//! and renamed into place, so a crashed seal never leaves something that
//! looks like a finished bag.
//!
//! Bag layout (payload under `data/`):
//! - `log/<scope>/<device>.jsonl`: the op log, byte for byte, or with Loro
//!   payloads removed (`SealOptions::strip_loro`), keeping `resolved`
//! - `exports/elan/<text>.eaf`, `exports/flextext/<text>.flextext`,
//!   `exports/lift/lexicon.lift`: current state in community formats
//! - `metadata/olac.xml` (one OLAC record), `metadata/imdi.xml` (minimal
//!   IMDI; enrich when Person/Session entities land, roadmap item 11),
//!   `metadata/access.json` (access levels, people, consent)
//! - `metadata/third-party.txt`, only when the project holds reference
//!   lists made by others (SIL Semantic Domains, Concepticon concept lists):
//!   their copyright, license and source, since their text is in the log
//! - `spec/DATA-MODEL.md`, `spec/op-schema.json`: the package explains itself
//! - `media/<hash>`: every blob, copied untouched

mod bagit;
mod metadata;
pub mod sha256;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use wugs_core::Project;
use wugs_model::{json, Op, Value};

pub use bagit::verify;

/// Licensed reference data a project can hold, by the `origin` (or
/// `source`) prefix its entities carry.
const THIRD_PARTY: &[(&str, &str, &str)] = &[
    (
        "domain",
        "semdom:",
        "SIL Semantic Domains v4. © SIL Global. Licensed under the Creative Commons Attribution-ShareAlike 4.0 International License (https://creativecommons.org/licenses/by-sa/4.0/). Source: https://semdom.org (github.com/sillsdev/semdom). The domain labels and descriptions in this package's log and exports come from that list; adaptations must be shared under the same license.",
    ),
    (
        "concept-list",
        "concepticon:",
        "Concept lists from Concepticon (https://concepticon.clld.org), Max Planck Institute for Evolutionary Anthropology, Leipzig. Licensed under the Creative Commons Attribution 4.0 International License (https://creativecommons.org/licenses/by/4.0/). Each list's citation is stored on its concept-list entity.",
    ),
];

/// The notice for every third-party list the project holds, or None.
fn third_party_notice(project: &Project) -> Result<Option<String>, String> {
    let mut parts = Vec::new();
    for (kind, prefix, text) in THIRD_PARTY {
        let mut found = false;
        for id in project.proj.entities_of_kind(kind)? {
            let origin = project.proj.field_str(&id, "origin")?.unwrap_or_default();
            if origin.starts_with(prefix) {
                found = true;
                break;
            }
        }
        if found {
            parts.push(text.to_string());
        }
    }
    Ok((!parts.is_empty()).then(|| format!("Third-party data in this package\n\n{}\n", parts.join("\n\n"))))
}

const DATA_MODEL: &str = include_str!("../../../docs/DATA-MODEL.md");
const OP_SCHEMA: &str = include_str!("op-schema.json");

#[derive(Debug, Clone, Default)]
pub struct SealOptions {
    /// Drop the `loro` CRDT payload from every text-edit op in the sealed
    /// copy of the log. `resolved` stays, so the log still replays to the
    /// same text with no Loro dependency.
    pub strip_loro: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SealReport {
    /// payload files
    pub files: usize,
    /// payload bytes
    pub bytes: u64,
    pub texts: usize,
    pub lexemes: usize,
    pub ops: usize,
    pub loro_stripped: usize,
    /// unparseable log lines before the last line of their file, kept as they are
    pub corrupt_lines: usize,
    /// symlinks under log/ or blobs/, skipped rather than followed
    pub symlinks_skipped: usize,
    /// some source, text or copied log scope is not `open`
    pub contains_restricted: bool,
}

/// Seal `project` into a new bag at `out`. Refuses an existing `out`:
/// sealing never replaces anything, so each seal targets a new directory.
pub fn seal(project: &Project, out: &Path, opts: &SealOptions) -> Result<SealReport, String> {
    if out.symlink_metadata().is_ok() {
        return Err(format!("{} already exists; seal into a new directory", out.display()));
    }
    let name = out.file_name().ok_or_else(|| format!("{}: no directory name", out.display()))?;
    let parent = out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    remove_stale_staging(parent, name)?;
    let staging = parent.join(format!("{}{}", staging_prefix(name), wugs_model::ulid()));

    let result = write_bag(project, &staging, opts).and_then(|report| {
        verify(&staging).map_err(|e| format!("the new bag failed verification: {e}"))?;
        bagit::sync_tree(&staging)?;
        // rename(2) silently replaces an empty directory, so re-check.
        if out.symlink_metadata().is_ok() {
            return Err(format!("{} appeared while sealing; seal into a new directory", out.display()));
        }
        fs::rename(&staging, out).map_err(|e| format!("{}: {e}", out.display()))?;
        bagit::sync_dir(parent)?;
        Ok(report)
    });
    if result.is_err() {
        fs::remove_dir_all(&staging).ok();
    }
    result
}

fn staging_prefix(name: &OsStr) -> String {
    format!(".{}.partial-", name.to_string_lossy())
}

/// A crashed seal into `out` leaves `.<name>.partial-<ulid>` beside it.
/// Remove only those; another target's staging (`.<name>.x.partial-…`)
/// never has a bare ULID after this prefix.
fn remove_stale_staging(parent: &Path, name: &OsStr) -> Result<(), String> {
    let prefix = staging_prefix(name);
    for entry in fs::read_dir(parent).map_err(|e| format!("{}: {e}", parent.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_name = entry.file_name();
        let Some(rest) = file_name.to_str().and_then(|n| n.strip_prefix(&prefix)) else { continue };
        let is_ulid = rest.len() == 26 && rest.bytes().all(|b| b.is_ascii_alphanumeric());
        if is_ulid && entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            fs::remove_dir_all(entry.path()).map_err(|e| format!("{}: {e}", entry.path().display()))?;
        }
    }
    Ok(())
}

fn write_bag(project: &Project, staging: &Path, opts: &SealOptions) -> Result<SealReport, String> {
    let mut bag = bagit::BagWriter::new(staging)?;
    let date = utc_date(wugs_model::id::now_millis());

    let log = copy_log(project, &mut bag, opts)?;

    let mut texts = Vec::new();
    let mut stems = BTreeSet::new();
    for id in live_of_kind(project, "text")? {
        // Text ids arrive by sync, so they name files only once sanitized.
        let base = wugs_oplog::sanitize(&id);
        let mut stem = base.clone();
        for n in 2.. {
            if stems.insert(stem.clone()) {
                break;
            }
            stem = format!("{base}-{n}");
        }
        let eaf = format!("exports/elan/{stem}.eaf");
        bag.put(&eaf, wugs_elan::export_eaf(project, &id)?.as_bytes())?;
        let flextext = format!("exports/flextext/{stem}.flextext");
        bag.put(&flextext, wugs_flex::export_flextext(project, &id)?.as_bytes())?;
        let title = project.proj.field_str(&id, "title")?.unwrap_or_else(|| id.clone());
        texts.push(metadata::TextEntry { id, title, files: vec![eaf, flextext] });
    }
    let lexemes = live_of_kind(project, "lexeme")?.len();
    if lexemes > 0 {
        bag.put("exports/lift/lexicon.lift", wugs_flex::export_lift(project)?.as_bytes())?;
    }

    let title = project_title(project)?;
    bag.put("metadata/olac.xml", metadata::olac(project, &title, &date, lexemes > 0)?.as_bytes())?;
    bag.put("metadata/imdi.xml", metadata::imdi(&texts, &date).as_bytes())?;
    let (access, contains_restricted) = metadata::access(project, &log.scopes)?;
    bag.put("metadata/access.json", access.as_bytes())?;
    if let Some(notice) = third_party_notice(project)? {
        bag.put("metadata/third-party.txt", notice.as_bytes())?;
    }
    bag.put("spec/DATA-MODEL.md", DATA_MODEL.as_bytes())?;
    bag.put("spec/op-schema.json", OP_SCHEMA.as_bytes())?;

    let blobs = project.root.join("blobs");
    let mut media = Vec::new();
    let symlinks_skipped = log.symlinks + bagit::walk(&blobs, &blobs, &mut media)?;
    media.sort();
    for rel in media {
        bag.copy(&format!("media/{rel}"), &blobs.join(&rel))?;
    }

    let report = SealReport {
        files: bag.payload_files(),
        bytes: bag.payload_bytes(),
        texts: texts.len(),
        lexemes,
        ops: log.ops,
        loro_stripped: log.loro_stripped,
        corrupt_lines: log.corrupt_lines,
        symlinks_skipped,
        contains_restricted,
    };
    let mut info = vec![
        ("Source-Organization", "wugs".to_string()),
        ("External-Identifier", project.project_id.clone()),
        ("External-Description", title),
        ("Bagging-Date", date),
        ("Bag-Software-Agent", format!("wugs-seal/{}", env!("CARGO_PKG_VERSION"))),
        ("Wugs-Text-Count", report.texts.to_string()),
        ("Wugs-Lexeme-Count", report.lexemes.to_string()),
        ("Wugs-Op-Count", report.ops.to_string()),
        ("Wugs-Loro-Stripped", report.loro_stripped.to_string()),
        ("Wugs-Contains-Restricted", report.contains_restricted.to_string()),
    ];
    if report.corrupt_lines > 0 {
        info.push(("Wugs-Corrupt-Lines", report.corrupt_lines.to_string()));
    }
    if report.symlinks_skipped > 0 {
        info.push(("Wugs-Symlinks-Skipped", report.symlinks_skipped.to_string()));
    }
    bag.finish(&info)?;
    Ok(report)
}

#[derive(Default)]
struct LogCopy {
    ops: usize,
    loro_stripped: usize,
    corrupt_lines: usize,
    symlinks: usize,
    /// scope directory names under log/
    scopes: BTreeSet<String>,
}

/// Copy every log file into `log/`. Sealing is the rescue path, so a
/// damaged log never stops it: unparseable lines are counted and kept.
fn copy_log(project: &Project, bag: &mut bagit::BagWriter, opts: &SealOptions) -> Result<LogCopy, String> {
    let log_root = project.log.log_root();
    let mut files = Vec::new();
    let mut copy = LogCopy { symlinks: bagit::walk(&log_root, &log_root, &mut files)?, ..LogCopy::default() };
    files.sort();
    for rel in files.iter().filter(|f| f.ends_with(".jsonl")) {
        let bytes = fs::read(log_root.join(rel)).map_err(|e| format!("log/{rel}: {e}"))?;
        if let Some((scope, _)) = rel.split_once('/') {
            copy.scopes.insert(scope.to_string());
        }
        let lines: Vec<&[u8]> = bytes.split(|&b| b == b'\n').filter(|l| !l.trim_ascii().is_empty()).collect();
        let mut stripped = Vec::with_capacity(bytes.len());
        for (i, line) in lines.iter().enumerate() {
            match std::str::from_utf8(line).ok().filter(|l| Op::from_json(l).is_ok()) {
                Some(op_line) => {
                    copy.ops += 1;
                    match opts.strip_loro.then(|| strip_loro(op_line)).flatten() {
                        Some(s) => {
                            copy.loro_stripped += 1;
                            stripped.extend_from_slice(s.as_bytes());
                        }
                        None => stripped.extend_from_slice(line),
                    }
                }
                // A torn final line is a crash mid-append; replay never
                // applied it (wugs-oplog), so the stripped copy drops it.
                None if i + 1 == lines.len() => continue,
                None => {
                    copy.corrupt_lines += 1;
                    stripped.extend_from_slice(line);
                }
            }
            stripped.push(b'\n');
        }
        let out = if opts.strip_loro { stripped } else { bytes };
        bag.put(&format!("log/{rel}"), &out)?;
    }
    Ok(copy)
}

/// The line with `value.loro` removed from a text-edit op, every other
/// field kept as written (unknown ones included); None if nothing to strip.
fn strip_loro(line: &str) -> Option<String> {
    let mut op = json::parse(line).ok()?;
    let Value::Obj(fields) = &mut op else { return None };
    if fields.get("op").and_then(Value::as_str) != Some("text-edit") {
        return None;
    }
    let Some(Value::Obj(value)) = fields.get_mut("value") else { return None };
    value.remove("loro")?;
    Some(op.to_json())
}

fn project_title(project: &Project) -> Result<String, String> {
    let text = fs::read_to_string(project.root.join("project.json")).map_err(|e| format!("project.json: {e}"))?;
    let v = json::parse(&text).map_err(|e| format!("project.json: {e}"))?;
    Ok(v.get_str("title")
        .map(str::to_string)
        .unwrap_or_else(|| format!("wugs project {}", project.project_id)))
}

pub(crate) fn live_of_kind(project: &Project, kind: &str) -> Result<Vec<String>, String> {
    Ok(all_of_kind(project, kind)?.into_iter().filter(|(_, deleted)| !deleted).map(|(id, _)| id).collect())
}

/// Every entity of `kind`, tombstoned ones included, as (id, deleted).
pub(crate) fn all_of_kind(project: &Project, kind: &str) -> Result<Vec<(String, bool)>, String> {
    let mut s = project.proj.db.prepare("SELECT id, tombstone FROM entities WHERE kind=?1 ORDER BY id")?;
    s.bind_text(1, kind)?;
    let mut out = Vec::new();
    while s.step()? {
        out.push((s.col_text(0), s.col_i64(1) != 0));
    }
    Ok(out)
}

pub(crate) fn fields_of(project: &Project, id: &str) -> Result<BTreeMap<String, Value>, String> {
    let mut s = project.proj.db.prepare("SELECT field, value FROM fields WHERE target=?1 ORDER BY field")?;
    s.bind_text(1, id)?;
    let mut out = BTreeMap::new();
    while s.step()? {
        let v = json::parse(&s.col_text(1)).map_err(|e| e.to_string())?;
        out.insert(s.col_text(0), v);
    }
    Ok(out)
}

/// `YYYY-MM-DD` in UTC (days-to-civil, H. Hinnant).
fn utc_date(millis: u64) -> String {
    let z = (millis / 86_400_000) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_dates() {
        assert_eq!(utc_date(0), "1970-01-01");
        assert_eq!(utc_date(951_782_400_000), "2000-02-29");
        assert_eq!(utc_date(1_793_923_200_000), "2026-11-06");
    }

    #[test]
    fn op_schema_parses() {
        let v = json::parse(OP_SCHEMA).unwrap();
        assert_eq!(v.get_str("$schema"), Some("https://json-schema.org/draft/2020-12/schema"));
        let verbs = v.get("properties").and_then(|p| p.get("op")).and_then(|o| o.get("enum")).unwrap();
        let verbs: Vec<&str> = match verbs {
            Value::Arr(a) => a.iter().filter_map(Value::as_str).collect(),
            _ => panic!("verb enum is not an array"),
        };
        assert_eq!(verbs, wugs_model::op::VERBS);
        let prop = |k: &str| v.get("properties").and_then(|p| p.get(k)).unwrap();
        let heads = prop("heads");
        assert_eq!(heads.get_str("type"), Some("array"));
        assert_eq!(heads.get("items").and_then(|i| i.get_str("pattern")), prop("hlc").get_str("pattern"), "heads are hlcs");
    }
}
