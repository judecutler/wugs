//! Plain text in and out (Scrolls, SCROLLS.md §5 step 2, first form): a
//! `.txt` becomes a text with one segment per sentence, and a text goes out
//! as one segment per line, the shape AntConc and most corpus tools read.
//!
//! How a file is cut into segments (`Cut` says which way, for the report):
//! - **By line**: a file with no blank line between its lines whose lines
//!   mostly end in a full stop (or that has three lines or fewer) comes in a
//!   segment per line: corpus files with a sentence per line, the commonest
//!   layout, come in as they are.
//! - **By sentence** otherwise: blank lines separate paragraphs (a file of
//!   hard-wrapped lines with no blank line is one paragraph); a line that
//!   ends in a full stop also ends its sentence, so a sentence-per-line block
//!   or a poem's lines keep their breaks; and inside a line, a sentence ends
//!   after `.` `!` `?` `…` (and the full stops of Arabic, Devanagari, Khmer,
//!   Myanmar, Ethiopic, Armenian and Tibetan) followed by a space and a word
//!   that does not start lower-case, so most abbreviations hold (`e.g. the`)
//!   while `Dr. Smith` is cut. CJK full stops cut with no space after them.
//!   Closing quotes and brackets stay with their sentence; a run of
//!   punctuation alone joins the sentence before it.
//!
//! Known limits: Thai and Lao have no sentence mark, so their paragraphs
//! come in whole; a script with no capitals, or an orthography written all
//! in lower case, is cut only at line ends and paragraph breaks, because a
//! lower-case word after a full stop never starts a sentence.

/// Sentence-final marks that cut only before a space.
const FINAL: [char; 13] = ['.', '!', '?', '…', '؟', '।', '॥', '۔', '។', '။', '።', '։', '།'];
/// Full stops of scripts written without spaces: they cut at once.
const FINAL_TIGHT: [char; 3] = ['。', '！', '？'];
/// Closing marks that stay with the sentence before them.
const CLOSERS: [char; 9] = ['"', '\'', ')', ']', '}', '»', '”', '’', '」'];

/// How a file was cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cut {
    ByLine,
    BySentence,
}

/// Whether a line ends a sentence: its last mark (past closing quotes and
/// brackets) is a full stop.
fn ends_sentence(line: &str) -> bool {
    line.trim_end().chars().rev().find(|c| !CLOSERS.contains(c)).is_some_and(|c| FINAL.contains(&c) || FINAL_TIGHT.contains(&c))
}

/// The segments of a plain-text file, in order, none empty, and how it was cut.
pub fn segments(text: &str) -> (Vec<String>, Cut) {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.lines().collect();
    let filled: Vec<&str> = lines.iter().copied().filter(|l| !l.trim().is_empty()).collect();
    // Blank lines only at the very start or end do not make paragraphs.
    let inner_blank = match (lines.iter().position(|l| !l.trim().is_empty()), lines.iter().rposition(|l| !l.trim().is_empty())) {
        (Some(a), Some(b)) => lines[a..=b].iter().any(|l| l.trim().is_empty()),
        _ => false,
    };
    let ending = filled.iter().filter(|l| ends_sentence(l)).count();
    // Hard-wrapped prose (few lines end a sentence) is a paragraph, not a line list.
    let line_list = filled.len() <= 3 || ending * 10 >= filled.len() * 3;
    if !inner_blank && line_list {
        return (filled.iter().map(|l| collapse(l)).filter(|l| !l.is_empty()).collect(), Cut::ByLine);
    }
    let mut out = Vec::new();
    for para in lines.split(|l| l.trim().is_empty()) {
        // Within a paragraph, a line that ends in a full stop ends its sentence.
        let mut run: Vec<&str> = Vec::new();
        for line in para {
            run.push(line);
            if ends_sentence(line) {
                out.extend(sentences(&collapse(&run.join(" "))));
                run.clear();
            }
        }
        if !run.is_empty() {
            out.extend(sentences(&collapse(&run.join(" "))));
        }
    }
    (merge_bare_punctuation(out), Cut::BySentence)
}

/// A piece with no letter or digit (a lone `...`) joins the one before it.
fn merge_bare_punctuation(pieces: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in pieces {
        let bare = !p.chars().any(char::is_alphanumeric);
        match out.last_mut() {
            Some(prev) if bare => {
                prev.push(' ');
                prev.push_str(&p);
            }
            _ => out.push(p),
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

    fn cut(text: &str) -> Vec<String> {
        segments(text).0
    }

    #[test]
    fn a_line_per_sentence_file_comes_in_as_it_is() {
        assert_eq!(segments("One sentence. Still the same line.\nSecond line\n\n"), (vec!["One sentence. Still the same line.".to_string(), "Second line".into()], Cut::ByLine));
        assert_eq!(cut("\u{feff}a\r\nb\r\n"), vec!["a", "b"]);
        assert!(cut("").is_empty());
        // A lower-case, tokenized corpus with a blank line between documents keeps its lines.
        assert_eq!(cut("the cat sat .\nthe dog ran .\n\nanother doc here .\n"), vec!["the cat sat .", "the dog ran .", "another doc here ."]);
    }

    #[test]
    fn hard_wrapped_prose_is_one_paragraph() {
        let text = "The fox came to the well and\nlooked down into the water for\na long time. Then it left the\nfield and went home.\n";
        assert_eq!(segments(text), (vec!["The fox came to the well and looked down into the water for a long time.".to_string(), "Then it left the field and went home.".into()], Cut::BySentence));
    }

    #[test]
    fn paragraphs_are_cut_into_sentences() {
        let text = "The fox came.  It drank\nfrom the well! Did it? \"Yes,\" she said. \"It did.\" Then, e.g. the cat\ncame too.\n\nA new paragraph… With two sentences. ...";
        assert_eq!(
            cut(text),
            vec!["The fox came.", "It drank from the well!", "Did it?", "\"Yes,\" she said.", "\"It did.\"", "Then, e.g. the cat came too.", "A new paragraph…", "With two sentences. ..."]
        );
        // A poem's lines keep their breaks.
        assert_eq!(cut("Line one.\nLine two.\n\nNext stanza.\n"), vec!["Line one.", "Line two.", "Next stanza."]);
    }

    #[test]
    fn other_scripts_cut_at_their_own_full_stops() {
        assert_eq!(cut("一つ。二つ！\n\n三つ？"), vec!["一つ。", "二つ！", "三つ？"]);
        assert_eq!(cut("هل جاء؟ نعم جاء.\n\nx"), vec!["هل جاء؟", "نعم جاء.", "x"]);
        assert_eq!(cut("वह आया। वह गया।\n\nx"), vec!["वह आया।", "वह गया।", "x"]);
        assert_eq!(cut("ខ្ញុំទៅ។ គាត់មក។\n\nx"), vec!["ខ្ញុំទៅ។", "គាត់មក។", "x"]);
        assert_eq!(cut("Count 3. 4 came.\n\nx"), vec!["Count 3.", "4 came.", "x"]);
    }

    #[test]
    fn export_is_a_line_per_segment() {
        assert_eq!(export(&["a  b".into(), "".into(), "c\nd".into()]), "a b\nc d\n");
    }
}
