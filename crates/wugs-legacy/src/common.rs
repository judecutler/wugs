use std::collections::{BTreeMap, HashMap};
use std::fmt;

use wugs_core::Project;
use wugs_model::Value;

pub const F_ORIGIN: &str = "origin";

#[derive(Debug)]
pub enum ImportError {
    Format(String),
    Project(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Format(m) => write!(f, "invalid file: {m}"),
            ImportError::Project(m) => write!(f, "project error: {m}"),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<String> for ImportError {
    fn from(e: String) -> Self {
        ImportError::Project(e)
    }
}

pub fn format_err(msg: impl Into<String>) -> ImportError {
    ImportError::Format(msg.into())
}

/// Named counts of things read but not carried semantically, so a user sees
/// exactly what a round trip keeps only verbatim, or does not keep.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Counts(pub BTreeMap<String, usize>);

impl Counts {
    pub fn add(&mut self, what: &str) {
        *self.0.entry(what.to_string()).or_default() += 1;
    }
    pub fn total(&self) -> usize {
        self.0.values().sum()
    }
}

/// Decode a file's bytes: UTF-8 (with or without BOM) or BOM-marked UTF-16,
/// which Praat writes whenever a TextGrid holds non-ASCII text.
pub fn decode(bytes: &[u8]) -> Result<String, ImportError> {
    let utf16 = |be: bool| {
        let body = &bytes[2..];
        if !body.len().is_multiple_of(2) {
            return Err(format_err("UTF-16 file has an odd number of bytes"));
        }
        let units: Vec<u16> = body
            .chunks(2)
            .map(|c| if be { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) })
            .collect();
        String::from_utf16(&units).map_err(|_| format_err("invalid UTF-16"))
    };
    match bytes {
        [0xFE, 0xFF, ..] => utf16(true),
        [0xFF, 0xFE, ..] => utf16(false),
        _ => {
            let s = std::str::from_utf8(bytes).map_err(|e| format_err(format!("not UTF-8: {e}")))?;
            Ok(strip_bom(s).to_string())
        }
    }
}

pub fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{feff}').unwrap_or(s)
}

/// A string field, or None when absent or cleared (set to null).
pub fn opt_str(project: &Project, id: &str, field: &str) -> Result<Option<String>, String> {
    Ok(project.proj.field(id, field)?.and_then(|(v, _)| v.as_str().map(str::to_string)))
}

pub fn opt_value(project: &Project, id: &str, field: &str) -> Result<Option<Value>, String> {
    Ok(project.proj.field(id, field)?.map(|(v, _)| v).filter(|v| *v != Value::Null))
}

/// origin -> entity id for every live entity of `kind`.
pub fn origin_index(project: &Project, kind: &str) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();
    for id in project.proj.entities_of_kind(kind)? {
        if let Some(o) = opt_str(project, &id, F_ORIGIN)? {
            out.insert(o, id);
        }
    }
    Ok(out)
}

/// `base`, then `base#2`, `base#3`… for its later occurrences, so formats
/// without ids (duplicate SFM headwords, repeated Praat tier names) still get
/// one stable key per item in document order.
#[derive(Default)]
pub struct Occurrences(HashMap<String, usize>);

impl Occurrences {
    pub fn key(&mut self, base: String) -> String {
        let n = self.0.entry(base.clone()).or_default();
        *n += 1;
        if *n == 1 {
            base
        } else {
            format!("{base}#{n}")
        }
    }
}

pub fn fnv1a(s: &str) -> u64 {
    s.bytes()
        .fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}
