//! Audacity label tracks (File > Export > Labels): a line per label,
//! `start<TAB>end<TAB>label`, times in seconds. Audacity writes a second
//! line starting `\` for a label with a frequency range, which is skipped.
//! In as one text with a segment per label; out as the labels of one track.

use std::fmt::Write as _;

/// A label: its span in milliseconds and its text.
#[derive(Debug, PartialEq)]
pub struct Label {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

fn seconds(s: &str) -> Option<f64> {
    let s = s.trim();
    let ok = !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '.') && s.matches('.').count() <= 1;
    ok.then(|| s.parse::<f64>().ok()).flatten().filter(|v| v.is_finite())
}

fn label(line: &str) -> Option<Label> {
    let mut f = line.splitn(3, '\t');
    let start = seconds(f.next()?)?;
    let end = seconds(f.next()?)?;
    let text = f.next().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ");
    Some(Label { start: (start * 1000.0).round(), end: (end * 1000.0).round(), text })
}

/// Whether text is an Audacity label track: at least one label, and every
/// other non-empty line a frequency line (`\` first).
pub fn is_labels(text: &str) -> bool {
    let text = text.trim_start_matches('\u{feff}');
    let mut labels = 0;
    for line in text.lines().map(|l| l.trim_end_matches('\r')).filter(|l| !l.trim().is_empty()) {
        if line.starts_with('\\') {
            continue;
        }
        if label(line).is_none() {
            return false;
        }
        labels += 1;
    }
    labels > 0
}

/// Every label with text, in the file's order.
pub fn read(text: &str) -> Vec<Label> {
    text.trim_start_matches('\u{feff}').lines().map(|l| l.trim_end_matches('\r')).filter(|l| !l.starts_with('\\')).filter_map(label).filter(|l| !l.text.is_empty()).collect()
}

/// `(start, end, text)` rows in milliseconds as Audacity writes them: six decimals.
pub fn write(rows: &[(f64, f64, String)]) -> String {
    let mut out = String::new();
    for (start, end, text) in rows {
        let _ = writeln!(out, "{:.6}\t{:.6}\t{}", start / 1000.0, end / 1000.0, text.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    out
}

/// One text's timed, non-empty sentences (`segments` in time order, as
/// `one_track` gives them) as labels, and how many were left out.
pub fn export(p: &wugs_core::Project, segments: &[String]) -> Result<(String, usize), String> {
    let mut rows = Vec::new();
    let mut left_out = 0;
    for seg in segments {
        let content = p.proj.field_str(seg, "content")?.unwrap_or_default();
        match (p.proj.field_num(seg, "start")?, p.proj.field_num(seg, "end")?) {
            (Some(start), Some(end)) if end >= start && !content.trim().is_empty() => rows.push((start, end, content)),
            _ => left_out += 1,
        }
    }
    Ok((write(&rows), left_out))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "0.500000\t1.250000\twʌn\n\\\t200.000000\t4000.000000\n1.800000\t2.400000\ttuː  θɹiː\n3.000000\t3.000000\tclick\n4.000000\t4.500000\t\n";

    #[test]
    fn labels_are_read_and_written() {
        assert!(is_labels(FILE));
        assert!(!is_labels("word\tgloss\nkani\tcount\n"), "a table with a header is not labels");
        assert!(!is_labels("Hello there.\n"));
        assert!(!is_labels(""));
        let l = read(FILE);
        assert_eq!(l.len(), 3, "the empty label is left out");
        assert_eq!(l[0], Label { start: 500.0, end: 1250.0, text: "wʌn".into() });
        assert_eq!(l[1].text, "tuː θɹiː");
        assert_eq!(l[2].start, l[2].end, "a point label keeps its time");
        assert_eq!(write(&[(500.0, 1250.0, "wʌn".into())]), "0.500000\t1.250000\twʌn\n");
    }
}
