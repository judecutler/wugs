//! Subtitles: SubRip (`.srt`) and WebVTT (`.vtt`), the caption files video
//! editors, YouTube, Subtitle Edit and community media projects read and
//! write.
//!
//! - **In:** one text per file, a timed segment per cue on the utterance
//!   track, in the cue's order. A cue's lines are joined with a space;
//!   styling (`<i>`, `<b>`, `<font …>`, `{\an8}`) is dropped and a WebVTT
//!   voice tag (`<v Ana>`) is kept as the cue's speaker (`x.vtt.speaker`).
//!   Cue numbers and settings are not kept. A cue whose end is not after its
//!   start comes in untimed. A WebVTT file wugs wrote (its `NOTE` says so)
//!   gives each cue's second line back as the sentence's translation.
//! - **Out:** a text's timed sentences, in time order, each with its
//!   translation on a second line when it has one (so a viewer reads the
//!   language and its translation together); untimed sentences are left out
//!   and counted.

use std::fmt::Write as _;

/// A time as subtitle and spreadsheet files write it, in milliseconds:
/// seconds (`12.5`), or `m:ss`, `h:mm:ss` with `.` or `,` before the
/// fraction (`00:01:02,500`). None for anything else or a negative time.
pub fn clock(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    let mut total = 0.0;
    for (i, part) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        let part = if last { part.replacen(',', ".", 1) } else { part.to_string() };
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit() || (last && c == '.')) || part.matches('.').count() > 1 {
            return None;
        }
        let v: f64 = part.parse().ok()?;
        if i > 0 && v >= 60.0 {
            return None;
        }
        total = total * 60.0 + v;
    }
    Some((total * 1000.0).round())
}

#[derive(Debug, PartialEq)]
pub struct Cue {
    pub text: String,
    /// Start and end in milliseconds; None when the end is not after the start.
    pub span: Option<(f64, f64)>,
    /// A WebVTT voice (`<v Ana>`), when the cue names one.
    pub speaker: Option<String>,
    /// The cue's second line, when the file says its second lines are translations.
    pub translation: Option<String>,
}

/// The note wugs writes at the top of a WebVTT file, read back to split
/// each cue's sentence from its translation.
const TWO_LINE_NOTE: &str = "NOTE wugs: the second line of a cue is its translation";

/// Drop markup: `<…>` tags (keeping a voice's name aside) and `{\…}` overrides.
fn clean(line: &str, speaker: &mut Option<String>) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(i) = rest.find(['<', '{']) {
        out.push_str(&rest[..i]);
        let close = if rest[i..].starts_with('<') { '>' } else { '}' };
        let after = &rest[i + 1..];
        // A `{` that does not open an override (`{\…}`) is text.
        if close == '}' && !after.starts_with('\\') {
            out.push('{');
            rest = after;
            continue;
        }
        match after.find(close) {
            Some(j) => {
                let tag = &after[..j];
                if let Some(name) = tag.strip_prefix("v ").or_else(|| tag.strip_prefix("v.").and_then(|t| t.split_once(' ').map(|(_, n)| n))) {
                    let name = name.trim();
                    if !name.is_empty() && speaker.is_none() {
                        *speaker = Some(name.to_string());
                    }
                }
                rest = &after[j + 1..];
            }
            None => {
                out.push_str(&rest[i..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    // `&lt;` and friends in WebVTT text.
    out.replace("&lt;", "<").replace("&gt;", ">").replace("&nbsp;", " ").replace("&amp;", "&")
}

/// The `start --> end` line of a cue, in milliseconds.
fn timing(line: &str) -> Option<(f64, f64)> {
    let (a, b) = line.split_once("-->")?;
    let start = clock(a)?;
    // WebVTT settings follow the end time (`00:02.000 align:start`).
    let end = clock(b.split_whitespace().next()?)?;
    Some((start, end))
}

/// Every cue of a SubRip or WebVTT file, in order. Refused when no line
/// holds a timing (`… --> …`), so a stray `.srt` that is something else
/// says so.
pub fn read(input: &str) -> Result<Vec<Cue>, String> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input).replace("\r\n", "\n").replace('\r', "\n");
    let mut cues = Vec::new();
    let mut timed = false;
    let two_lines = input.split("\n\n").any(|b| b.trim() == TWO_LINE_NOTE);
    for block in input.split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        let Some(at) = lines.iter().position(|l| l.contains("-->")) else { continue };
        let Some((start, end)) = timing(lines[at]) else { continue };
        timed = true;
        let mut speaker = None;
        let text: Vec<String> = lines[at + 1..].iter().map(|l| clean(l, &mut speaker)).map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect();
        if text.is_empty() {
            continue;
        }
        let (text, translation) = match (two_lines, text.len()) {
            (true, 2..) => (text[0].clone(), Some(text[1..].join(" "))),
            _ => (text.join(" "), None),
        };
        cues.push(Cue { text, span: (end > start).then_some((start, end)), speaker, translation });
    }
    if !timed {
        return Err("no cue timings (a line like 00:00:01,000 --> 00:00:02,500), so this is not a subtitle file".into());
    }
    Ok(cues)
}

fn stamp(ms: f64, vtt: bool) -> String {
    let ms = ms.max(0.0).round() as u64;
    let (h, m, s, f) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    format!("{h:02}:{m:02}:{s:02}{}{f:03}", if vtt { '.' } else { ',' })
}

/// `(start, end, sentence, translation)` rows as SubRip or WebVTT. A
/// sentence's line breaks become spaces; WebVTT escapes `<` `>` `&`, and an
/// empty line inside a cue never happens, since it would end the cue.
pub fn write(rows: &[(f64, f64, String, Option<String>)], vtt: bool) -> String {
    let mut out = String::new();
    if vtt {
        out.push_str("WEBVTT\n\n");
        if rows.iter().any(|r| r.3.as_deref().is_some_and(|t| !t.trim().is_empty())) {
            out.push_str(TWO_LINE_NOTE);
            out.push_str("\n\n");
        }
    }
    let one_line = |s: &str| {
        let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
        if vtt {
            s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        } else {
            s
        }
    };
    for (i, (start, end, sentence, translation)) in rows.iter().enumerate() {
        if !vtt {
            let _ = writeln!(out, "{}", i + 1);
        }
        let _ = writeln!(out, "{} --> {}", stamp(*start, vtt), stamp(*end, vtt));
        let _ = writeln!(out, "{}", one_line(sentence));
        if let Some(t) = translation.as_deref().map(one_line).filter(|t| !t.is_empty()) {
            let _ = writeln!(out, "{t}");
        }
        out.push('\n');
    }
    out
}

/// One text's timed sentences (`segments`, in time order, as `one_track`
/// gives them) with their first translation, and how many were untimed or
/// empty and left out.
pub fn export(p: &wugs_core::Project, segments: &[String], vtt: bool) -> Result<(String, usize), String> {
    let mut rows = Vec::new();
    let mut left_out = 0;
    for seg in segments {
        let content = p.proj.field_str(seg, "content")?.unwrap_or_default();
        let (Some(start), Some(end)) = (p.proj.field_num(seg, "start")?, p.proj.field_num(seg, "end")?) else {
            left_out += 1;
            continue;
        };
        if content.trim().is_empty() || end <= start {
            left_out += 1;
            continue;
        }
        let translation = crate::interchange::translations_of(p, seg)?.into_iter().map(|t| t.1).next();
        rows.push((start, end, content, translation));
    }
    Ok((write(&rows, vtt), left_out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_clock_times() {
        assert_eq!(clock("12.5"), Some(12_500.0));
        assert_eq!(clock("00:01:02,500"), Some(62_500.0));
        assert_eq!(clock("01:02.250"), Some(62_250.0));
        assert_eq!(clock("1:00:00"), Some(3_600_000.0));
        for bad in ["", "abc", "1:75", "-3", "1:2:3:4", "1..2", "1,5:00"] {
            assert_eq!(clock(bad), None, "{bad}");
        }
    }

    #[test]
    fn reads_srt_and_vtt() {
        let srt = "\u{feff}1\r\n00:00:01,000 --> 00:00:02,500\r\n<i>wʌn</i> {\\an8}tuː\r\nθɹiː\r\n\r\n2\r\n00:00:03,000 --> 00:00:03,000\r\nfɔɹ\r\n\r\n3\r\n00:00:04,000 --> 00:00:05,000\r\n\r\n";
        let cues = read(srt).unwrap();
        assert_eq!(cues, vec![Cue { text: "wʌn tuː θɹiː".into(), span: Some((1000.0, 2500.0)), speaker: None, translation: None }, Cue { text: "fɔɹ".into(), span: None, speaker: None, translation: None }]);
        let vtt = "WEBVTT - counting\n\nNOTE made by hand\n\nintro\n00:01.000 --> 00:02.000 align:start position:10%\n<v Ana>one &amp; two</v>\n\n00:03.000 --> 00:04.000\n<v.loud Ben>{three}</v>\n";
        let cues = read(vtt).unwrap();
        assert_eq!(cues[0], Cue { text: "one & two".into(), span: Some((1000.0, 2000.0)), speaker: Some("Ana".into()), translation: None });
        assert_eq!(cues[1], Cue { text: "{three}".into(), span: Some((3000.0, 4000.0)), speaker: Some("Ben".into()), translation: None });
        assert!(read("just a note\nwith lines\n").is_err());
    }

    #[test]
    fn writes_srt_and_vtt() {
        let rows = vec![(1000.0, 2500.0, "wʌn  tuː".to_string(), Some("one two".to_string())), (3_723_004.0, 3_724_000.0, "a<b".to_string(), None)];
        assert_eq!(write(&rows, false), "1\n00:00:01,000 --> 00:00:02,500\nwʌn tuː\none two\n\n2\n01:02:03,004 --> 01:02:04,000\na<b\n\n");
        assert_eq!(write(&rows, true), "WEBVTT\n\nNOTE wugs: the second line of a cue is its translation\n\n00:00:01.000 --> 00:00:02.500\nwʌn tuː\none two\n\n01:02:03.004 --> 01:02:04.000\na&lt;b\n\n");
        // What wugs writes reads back the same.
        let back = read(&write(&rows, true)).unwrap();
        assert_eq!(back[0], Cue { text: "wʌn tuː".into(), span: Some((1000.0, 2500.0)), speaker: None, translation: Some("one two".into()) });
        assert_eq!(back[1], Cue { text: "a<b".into(), span: Some((3_723_004.0, 3_724_000.0)), speaker: None, translation: None });
        // SubRip has no notes, so its two lines come back as one.
        assert_eq!(read(&write(&rows, false)).unwrap()[0].text, "wʌn tuː one two");
    }
}
