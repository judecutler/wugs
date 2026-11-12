//! Plain text in and out (Scrolls, SCROLLS.md §5 step 2, first form): a
//! `.txt` becomes a text with one segment per sentence, and a text goes out
//! as one segment per line, the shape AntConc and most corpus tools read.
//!
//! How a file is cut into segments:
//! - A file with no blank line is read one segment per line: corpus files
//!   with a sentence per line (the commonest layout) come in as they are.
//! - Otherwise blank lines separate paragraphs; a paragraph's lines are
//!   joined and cut into sentences after `.` `!` `?` `…` (and the CJK,
//!   Arabic and Devanagari full stops) followed by a space and a capital,
//!   a digit, an opening quote or bracket, or another script's letter;
//!   closing quotes and brackets stay with their sentence. A lower-case
//!   word after a full stop (`e.g. the`) never starts a sentence, so most
//!   abbreviations hold; `Dr. Smith` is cut, a known limit.
//! CJK full stops cut even with no space after them.

/// Sentence-final marks that cut only before a space.
const FINAL: [char; 7] = ['.', '!', '?', '…', '؟', '।', '۔'];
/// Full stops of scripts written without spaces: they cut at once.
const FINAL_TIGHT: [char; 3] = ['。', '！', '？'];
/// Closing marks that stay with the sentence before them.
const CLOSERS: [char; 9] = ['"', '\'', ')', ']', '}', '»', '”', '’', '」'];

/// The segments of a plain-text file, in order, none empty.
pub fn segments(text: &str) -> Vec<String> {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.lines().collect();
    let has_blank = lines.iter().any(|l| l.trim().is_empty()) && lines.iter().filter(|l| !l.trim().is_empty()).count() > 0;
    // Blank lines only at the very start or end do not make paragraphs.
    let inner_blank = {
        let first = lines.iter().position(|l| !l.trim().is_empty());
        let last = lines.iter().rposition(|l| !l.trim().is_empty());
        match (first, last) {
            (Some(a), Some(b)) => lines[a..=b].iter().any(|l| l.trim().is_empty()),
            _ => false,
        }
    };
    if !(has_blank && inner_blank) {
        return lines.iter().map(|l| collapse(l)).filter(|l| !l.is_empty()).collect();
    }
    let mut out = Vec::new();
    for para in text.split("\n").collect::<Vec<_>>().split(|l| l.trim().is_empty()) {
        let joined = collapse(&para.join(" "));
        if !joined.is_empty() {
            out.extend(sentences(&joined));
        }
    }
    out
}

/// Runs of whitespace as one space, trimmed.
fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One paragraph (already on one line) cut into sentences.
fn sentences(para: &str) -> Vec<String> {
    let chars: Vec<char> = para.chars().collect();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if FINAL_TIGHT.contains(&c) {
            let mut end = i + 1;
            while end < chars.len() && (CLOSERS.contains(&chars[end]) || FINAL_TIGHT.contains(&chars[end])) {
                end += 1;
            }
            push(&mut out, &chars[start..end]);
            start = end;
            i = end;
            continue;
        }
        if FINAL.contains(&c) {
            let mut end = i + 1;
            while end < chars.len() && (FINAL.contains(&chars[end]) || CLOSERS.contains(&chars[end])) {
                end += 1;
            }
            if end < chars.len() && chars[end] == ' ' && starts_sentence(chars.get(end + 1).copied()) {
                push(&mut out, &chars[start..end]);
                start = end + 1;
                i = end + 1;
                continue;
            }
            i = end;
            continue;
        }
        i += 1;
    }
    push(&mut out, &chars[start..]);
    out
}

fn push(out: &mut Vec<String>, piece: &[char]) {
    let s: String = piece.iter().collect::<String>().trim().to_string();
    if !s.is_empty() {
        out.push(s);
    }
}

/// Whether a sentence may start with this character: not a lower-case letter.
fn starts_sentence(c: Option<char>) -> bool {
    match c {
        None => false,
        Some(c) => !c.is_lowercase(),
    }
}

/// A text as plain text: its segments on the given tier, one per line, in order.
pub fn export(lines: &[String]) -> String {
    let mut out = String::new();
    for l in lines {
        let one = collapse(l);
        if !one.is_empty() {
            out.push_str(&one);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_per_sentence_file_comes_in_as_it_is() {
        assert_eq!(segments("One sentence. Still the same line.\nSecond line\n\n"), vec!["One sentence. Still the same line.", "Second line"]);
        assert_eq!(segments("\u{feff}a\r\nb\r\n"), vec!["a", "b"]);
        assert!(segments("").is_empty());
    }

    #[test]
    fn paragraphs_are_cut_into_sentences() {
        let text = "The fox came.  It drank\nfrom the well! Did it? \"Yes,\" she said. \"It did.\" Then, e.g. the cat\ncame too.\n\nA new paragraph… With two sentences.";
        assert_eq!(
            segments(text),
            vec!["The fox came.", "It drank from the well!", "Did it?", "\"Yes,\" she said.", "\"It did.\"", "Then, e.g. the cat came too.", "A new paragraph…", "With two sentences."]
        );
    }

    #[test]
    fn other_scripts_cut_at_their_own_full_stops() {
        assert_eq!(segments("一つ。二つ！\n\n三つ？"), vec!["一つ。", "二つ！", "三つ？"]);
        assert_eq!(segments("هل جاء؟ نعم جاء.\n\nx"), vec!["هل جاء؟", "نعم جاء.", "x"]);
        assert_eq!(segments("वह आया। वह गया।\n\nx"), vec!["वह आया।", "वह गया।", "x"]);
        // Numbers and IPA after a full stop.
        assert_eq!(segments("Count 3. 4 came.\n\nx"), vec!["Count 3.", "4 came.", "x"]);
    }

    #[test]
    fn export_is_a_line_per_segment() {
        assert_eq!(export(&["a  b".into(), "".into(), "c\nd".into()]), "a b\nc d\n");
    }
}
