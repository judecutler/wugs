//! CoNLL-U as text: a strict reader that names every problem by line, and a
//! writer that gives back what the reader read, byte for byte, for a file in
//! the order the format asks for (a range line before its first word, an
//! empty node after the word it follows). No project here; pure.
//!
//! Format: <https://universaldependencies.org/format.html>. Ten columns
//! separated by tabs: ID FORM LEMMA UPOS XPOS FEATS HEAD DEPREL DEPS MISC.

use std::fmt;

/// The nine columns after ID, as written (`_` for none).
pub type Cols = [String; 9];

pub const FORM: usize = 0;
pub const LEMMA: usize = 1;
pub const UPOS: usize = 2;
pub const XPOS: usize = 3;
pub const FEATS: usize = 4;
pub const HEAD: usize = 5;
pub const DEPREL: usize = 6;
pub const DEPS: usize = 7;
pub const MISC: usize = 8;

/// The column names after ID, for the analysis field's keys and reports.
pub const NAMES: [&str; 9] = ["form", "lemma", "upos", "xpos", "feats", "head", "deprel", "deps", "misc"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// A syntactic word, numbered from 1 within its sentence.
    Word { id: u32, cols: Cols },
    /// A multi-word token `first-last`: the surface form of several words.
    Range { first: u32, last: u32, cols: Cols },
    /// An empty node `major.minor` (enhanced dependencies), after word `major`.
    Empty { major: u32, minor: u32, cols: Cols },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sentence {
    /// Each comment line without its `#`, exactly as written (`" text = …"`).
    pub comments: Vec<String>,
    pub lines: Vec<Line>,
}

impl Sentence {
    /// The value of a `# key = value` comment, if there is one.
    pub fn comment(&self, key: &str) -> Option<&str> {
        self.comments.iter().find_map(|c| comment_value(c, key))
    }

    /// The syntactic words' lines, in order.
    pub fn words(&self) -> impl Iterator<Item = (u32, &Cols)> {
        self.lines.iter().filter_map(|l| match l {
            Line::Word { id, cols } => Some((*id, cols)),
            _ => None,
        })
    }

    /// The sentence as it reads: range forms stand for their words, and a
    /// space follows every token unless its MISC says `SpaceAfter=No`.
    pub fn surface(&self) -> String {
        let mut out = String::new();
        let mut covered_to = 0;
        for line in &self.lines {
            let cols = match line {
                Line::Range { last, cols, .. } => {
                    covered_to = *last;
                    cols
                }
                Line::Word { id, cols } if *id > covered_to => cols,
                _ => continue,
            };
            out.push_str(&cols[FORM]);
            if !misc_has(&cols[MISC], "SpaceAfter", "No") {
                out.push(' ');
            }
        }
        out.trim_end().to_string()
    }
}

/// `" text = Hello"` → `Some("Hello")` for key `text`.
pub fn comment_value<'a>(comment: &'a str, key: &str) -> Option<&'a str> {
    let rest = comment.trim_start().strip_prefix(key)?;
    let rest = rest.trim_start().strip_prefix('=')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// Whether a MISC column holds `key=value` among its `|`-separated items.
pub fn misc_has(misc: &str, key: &str, value: &str) -> bool {
    misc != "_" && misc.split('|').any(|item| item.split_once('=') == Some((key, value)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// 1-based line number in the file.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// Read a CoNLL-U file. Every problem is listed (at most 20), each with its
/// line; a file with any is refused whole, so nothing half-read comes in.
pub fn parse(input: &str) -> Result<Vec<Sentence>, Vec<Problem>> {
    let mut sentences = Vec::new();
    let mut problems = Vec::new();
    let mut current = Sentence::default();
    let mut current_start = 1;
    let push = |s: Sentence, start: usize, problems: &mut Vec<Problem>, out: &mut Vec<Sentence>| {
        if s.lines.is_empty() && s.comments.is_empty() {
            return;
        }
        if s.lines.is_empty() {
            problems.push(Problem { line: start, message: "comments with no words under them".into() });
            return;
        }
        check_sentence(&s, start, problems);
        out.push(s);
    };
    for (i, raw) in input.trim_start_matches('\u{feff}').split('\n').enumerate() {
        let n = i + 1;
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.trim().is_empty() {
            push(std::mem::take(&mut current), current_start, &mut problems, &mut sentences);
            current_start = n + 1;
            continue;
        }
        if current.lines.is_empty() && current.comments.is_empty() {
            current_start = n;
        }
        if let Some(c) = line.strip_prefix('#') {
            if !current.lines.is_empty() {
                problems.push(Problem { line: n, message: "a comment among the word lines; comments go above a sentence".into() });
            }
            current.comments.push(c.to_string());
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 10 {
            let hint = if fields.len() == 1 && line.contains(' ') { " (columns must be separated by tabs, not spaces)" } else { "" };
            problems.push(Problem { line: n, message: format!("{} columns where CoNLL-U has 10{hint}", fields.len()) });
            continue;
        }
        let cols: Cols = std::array::from_fn(|k| fields[k + 1].to_string());
        if cols.iter().any(|c| c.is_empty()) {
            problems.push(Problem { line: n, message: "an empty column; write _ for none".into() });
            continue;
        }
        match read_id(fields[0]) {
            Some(Id::Word(id)) => current.lines.push(Line::Word { id, cols }),
            Some(Id::Range(first, last)) => current.lines.push(Line::Range { first, last, cols }),
            Some(Id::Empty(major, minor)) => current.lines.push(Line::Empty { major, minor, cols }),
            None => problems.push(Problem { line: n, message: format!("«{}» is not a word number, a range like 1-2 or an empty node like 3.1", fields[0]) }),
        }
        if problems.len() >= 20 {
            break;
        }
    }
    if problems.len() < 20 {
        push(current, current_start, &mut problems, &mut sentences);
    }
    problems.truncate(20);
    if problems.is_empty() { Ok(sentences) } else { Err(problems) }
}

enum Id {
    Word(u32),
    Range(u32, u32),
    Empty(u32, u32),
}

fn read_id(s: &str) -> Option<Id> {
    let num = |t: &str| (!t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) && !(t.len() > 1 && t.starts_with('0'))).then(|| t.parse::<u32>().ok()).flatten();
    if let Some((a, b)) = s.split_once('-') {
        return Some(Id::Range(num(a)?, num(b)?));
    }
    if let Some((a, b)) = s.split_once('.') {
        return Some(Id::Empty(num(a)?, num(b)?));
    }
    Some(Id::Word(num(s)?))
}

/// Words numbered 1, 2, 3 …; ranges over words that follow; empty nodes in
/// order after their word; heads naming a word of the sentence. The file's
/// line numbers count from the sentence's first line.
fn check_sentence(s: &Sentence, start: usize, problems: &mut Vec<Problem>) {
    let first_line = start + s.comments.len();
    let words = s.words().count() as u32;
    let mut expect = 1;
    let mut last_empty = (0, 0);
    let mut open_range: Option<(u32, u32)> = None;
    for (k, line) in s.lines.iter().enumerate() {
        let n = first_line + k;
        let mut bad = |m: String| problems.push(Problem { line: n, message: m });
        match line {
            Line::Word { id, cols } => {
                if *id != expect {
                    bad(format!("word {id} where word {expect} comes next"));
                }
                expect = id + 1;
                last_empty = (*id, 0);
                if let Some((_, last)) = open_range {
                    if *id >= last {
                        open_range = None;
                    }
                }
                if cols[HEAD] != "_" {
                    match cols[HEAD].parse::<u32>() {
                        Ok(h) if h <= words && !(cols[HEAD].len() > 1 && cols[HEAD].starts_with('0')) => {
                            if h == *id {
                                bad(format!("word {id} is its own head"));
                            }
                        }
                        _ => bad(format!("head «{}» is not a word of this sentence (0 to {words})", cols[HEAD])),
                    }
                }
            }
            Line::Range { first, last, .. } => {
                if *first != expect || last <= first {
                    bad(format!("range {first}-{last} must start at word {expect} and cover at least two words"));
                }
                if *last > words {
                    bad(format!("range {first}-{last} runs past the last word, {words}"));
                }
                if open_range.is_some() {
                    bad("a range inside another range".into());
                }
                open_range = Some((*first, *last));
            }
            Line::Empty { major, minor, .. } => {
                if *major != expect - 1 || *minor != last_empty.1 + 1 || last_empty.0 != *major {
                    bad(format!("empty node {major}.{minor} out of order (after word {}, the next is {}.{})", expect - 1, expect - 1, if last_empty.0 == expect - 1 { last_empty.1 + 1 } else { 1 }));
                }
                last_empty = (*major, *minor);
                if open_range.is_some() {
                    bad(format!("empty node {major}.{minor} inside a range"));
                }
            }
        }
    }
}

/// Write sentences back as CoNLL-U: comments, lines in order, a blank line
/// after each sentence.
pub fn write(sentences: &[Sentence]) -> String {
    let mut out = String::new();
    for s in sentences {
        for c in &s.comments {
            out.push('#');
            out.push_str(c);
            out.push('\n');
        }
        for line in &s.lines {
            let (id, cols) = match line {
                Line::Word { id, cols } => (id.to_string(), cols),
                Line::Range { first, last, cols } => (format!("{first}-{last}"), cols),
                Line::Empty { major, minor, cols } => (format!("{major}.{minor}"), cols),
            };
            out.push_str(&id);
            for c in cols {
                out.push('\t');
                out.push_str(c);
            }
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

/// A value safe for one column: no tabs or line breaks (they would break
/// the line), and `_` when empty.
pub fn column(s: &str) -> String {
    let cleaned: String = s.chars().map(|c| if matches!(c, '\t' | '\n' | '\r') { ' ' } else { c }).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() { "_".into() } else { cleaned.to_string() }
}

/// A MISC value: as [`column`], and no `|` (it separates MISC items).
pub fn misc_value(s: &str) -> String {
    column(&s.replace('|', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# sent_id = 1\n# text = Vámonos al mar.\n1-2\tVámonos\t_\t_\t_\t_\t_\t_\t_\t_\n1\tVamos\tir\tVERB\t_\tMood=Imp\t0\troot\t_\t_\n2\tnos\tnosotros\tPRON\t_\t_\t1\tobj\t_\t_\n3-4\tal\t_\t_\t_\t_\t_\t_\t_\t_\n3\ta\ta\tADP\t_\t_\t5\tcase\t_\t_\n4\tel\tel\tDET\t_\t_\t5\tdet\t_\t_\n5\tmar\tmar\tNOUN\t_\t_\t1\tobl\t_\tSpaceAfter=No\n6\t.\t.\tPUNCT\t_\t_\t1\tpunct\t_\t_\n\n";

    #[test]
    fn reads_and_writes_back_the_same_bytes() {
        let s = parse(SAMPLE).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].comment("text"), Some("Vámonos al mar."));
        assert_eq!(s[0].words().count(), 6);
        assert_eq!(write(&s), SAMPLE);
    }

    #[test]
    fn the_surface_joins_range_forms_and_honours_space_after() {
        let s = parse(SAMPLE).unwrap();
        assert_eq!(s[0].surface(), "Vámonos al mar.");
    }

    #[test]
    fn crlf_a_bom_and_no_final_blank_line_still_read() {
        let crlf = format!("\u{feff}{}", SAMPLE.trim_end().replace('\n', "\r\n"));
        assert_eq!(parse(&crlf).unwrap(), parse(SAMPLE).unwrap());
    }

    #[test]
    fn empty_nodes_sit_after_their_word() {
        let t = "1\tSue\tSue\tPROPN\t_\t_\t2\tnsubj\t2:nsubj\t_\n2\tlikes\tlike\tVERB\t_\t_\t0\troot\t0:root\t_\n2.1\tlikes\tlike\tVERB\t_\t_\t_\t_\t0:root\t_\n3\ttea\ttea\tNOUN\t_\t_\t2\tobj\t2.1:obj\t_\n\n";
        let s = parse(t).unwrap();
        assert!(matches!(s[0].lines[2], Line::Empty { major: 2, minor: 1, .. }));
        assert_eq!(write(&s), t);
    }

    #[test]
    fn every_problem_is_named_with_its_line() {
        let bad = "1\ta\t_\t_\t_\t_\t0\troot\t_\t_\n3\tb\t_\t_\t_\t_\t9\tdep\t_\t_\n4 c _ _\n\n# lonely\n";
        let p = parse(bad).unwrap_err();
        let lines: Vec<usize> = p.iter().map(|p| p.line).collect();
        assert!(lines.contains(&2), "{p:?}");
        assert!(lines.contains(&3), "{p:?}");
        assert!(p.iter().any(|p| p.message.contains("tabs")), "{p:?}");
        assert!(p.iter().any(|p| p.message.contains("head «9»")), "{p:?}");
        assert!(p.iter().any(|p| p.line == 5 && p.message.contains("no words")), "{p:?}");
    }

    #[test]
    fn ids_with_leading_zeros_or_backwards_ranges_are_refused() {
        assert!(parse("01\ta\t_\t_\t_\t_\t_\t_\t_\t_\n").is_err());
        assert!(parse("2-1\tab\t_\t_\t_\t_\t_\t_\t_\t_\n1\ta\t_\t_\t_\t_\t_\t_\t_\t_\n2\tb\t_\t_\t_\t_\t_\t_\t_\t_\n").is_err());
    }

    #[test]
    fn columns_never_carry_tabs_or_line_breaks() {
        assert_eq!(column(" a\tb\n"), "a b");
        assert_eq!(column(""), "_");
        assert_eq!(misc_value("x|y"), "x/y");
    }
}
