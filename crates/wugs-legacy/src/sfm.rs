//! Standard Format Marker files, the storage format of SIL Toolbox and
//! Shoebox: one `\marker value` field per line, continuation lines folded
//! into the field above, records opened by the database's record marker.

use crate::common::{format_err, strip_bom, ImportError};

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub marker: String,
    /// Continuation lines joined with `\n`; surrounding whitespace trimmed.
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SfmFile {
    /// Lines before the first record (`\_sh v3.0  400  MDF 4.0`, other
    /// `\_` lines), verbatim, trailing blank lines dropped.
    pub header: Vec<String>,
    /// Each record starts with the record marker.
    pub records: Vec<Vec<Field>>,
}

pub fn parse(input: &str, record_marker: &str) -> Result<SfmFile, ImportError> {
    let mut header = Vec::new();
    let mut records: Vec<Vec<Field>> = Vec::new();
    for (n, line) in strip_bom(input).lines().enumerate() {
        let Some(rest) = line.strip_prefix('\\') else {
            match records.last_mut().and_then(|r| r.last_mut()) {
                Some(f) => {
                    f.value.push('\n');
                    f.value.push_str(line.trim_end());
                }
                None if line.trim().is_empty() => header.push(line.to_string()),
                None => return Err(format_err(format!("line {}: text before the first marker", n + 1))),
            }
            continue;
        };
        let marker_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let marker = &rest[..marker_end];
        if marker.is_empty() {
            return Err(format_err(format!("line {}: backslash without a marker", n + 1)));
        }
        let field = Field { marker: marker.to_string(), value: rest[marker_end..].to_string() };
        if marker == record_marker {
            records.push(vec![field]);
        } else if let Some(r) = records.last_mut() {
            r.push(field);
        } else if marker.starts_with('_') {
            header.push(line.to_string());
        } else {
            return Err(format_err(format!("line {}: \\{marker} outside any \\{record_marker} record", n + 1)));
        }
    }
    for f in records.iter_mut().flatten() {
        f.value = f.value.trim().to_string();
    }
    while header.last().is_some_and(|l| l.trim().is_empty()) {
        header.pop();
    }
    Ok(SfmFile { header, records })
}

/// Render as Toolbox writes it: header, then each record after a blank line.
pub fn write(file: &SfmFile) -> String {
    let mut out = String::new();
    for h in &file.header {
        out.push_str(h);
        out.push('\n');
    }
    for r in &file.records {
        out.push('\n');
        for f in r {
            out.push('\\');
            out.push_str(&f.marker);
            if !f.value.is_empty() {
                out.push(' ');
                out.push_str(&f.value);
            }
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuation_lines_fold_and_round_trip() {
        let src = "\u{feff}\\_sh v3.0  400  MDF 4.0\n\n\\lx a\n\\nt one\ntwo\n\n\\lx b\n\\ge\n";
        let f = parse(src, "lx").unwrap();
        assert_eq!(f.header, vec!["\\_sh v3.0  400  MDF 4.0"]);
        assert_eq!(f.records[0][1].value, "one\ntwo", "continuation folded, record gap dropped");
        assert_eq!(f.records[1][1].value, "", "empty field kept");
        assert_eq!(parse(&write(&f), "lx").unwrap(), f, "write then parse is identity");
    }
}
