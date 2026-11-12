//! Plain text in and out (Scrolls, SCROLLS.md §5 step 2, first form): a
//! `.txt` becomes a text with one segment per sentence, and a text goes out
//! as one segment per line, the shape AntConc and most corpus tools read.
//!
//! How a file is cut into segments (`Cut` says which way, for the report):
//! - **Hard-wrapped prose** is found by how wrapping works: a wrapped line
//!   breaks where the next word would not have fit, so the line plus a space
//!   plus the next line's first word is longer than the file's longest line.
//!   When at least two lines, and at least 60% of the lines that do not end
//!   in a full stop, break that way, and the longest line has 20 characters
//!   or more, the file is prose: those lines join the next, and the joined
//!   text is cut into sentences. A line that ends in a full stop, or that
//!   would have fit more (an indented paragraph's last line), ends its run.
//! - **Otherwise each line stays a line.** It is cut into sentences too when
//!   the file has blank lines between its lines or most lines hold more than
//!   one sentence (a paragraph per line); a file of a sentence or utterance
//!   per line, punctuated or not (word lists, transcripts, tokenized corpora),
//!   comes in a segment per line as it is.
//! - Inside a line, a sentence ends after `.` `!` `?` `…` (and the full stops
//!   of Arabic, Devanagari, Khmer, Myanmar, Ethiopic, Armenian and Tibetan)
//!   followed by a space and a word that does not start lower-case, so most
//!   abbreviations hold (`e.g. the`) while `Dr. Smith` is cut. CJK full stops
//!   cut with no space after them. Closing quotes and brackets stay with
//!   their sentence; a run of punctuation alone joins the sentence before it.
//!
//! A full stop after a single letter (an initial, `n.`, `z. B.`), a short
//! number (`3. Oktober`) or a common abbreviation (`Dr.`, `ca.`, `cf.`)
//! never ends a sentence; a piece in single quotes (`'The dog ate.'`, a translation) stays with the
//! sentence before it, and a line that starts with an example number, a
//! timestamp or a speaker label (`A: `) is never cut and never joins the
//! line before.
//!
//! Known limits: Thai and Lao have no sentence mark, so their paragraphs
//! come in whole; an orthography written all in lower case is cut only at
//! line ends and paragraph breaks, because a lower-case word after a full
//! stop never starts a sentence; a sentence that really ends in a short
//! number or a listed abbreviation (`I counted to 3. Then`) is not cut.

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

/// Characters of scripts written without spaces between words (Han, kana,
/// CJK punctuation and full-width forms): a wrapped line of them joins the
/// next with nothing between, and a wrap is measured by one character.
fn tight(c: char) -> bool {
    matches!(c as u32, 0x3000..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF | 0x20000..=0x2FFFF)
}

/// A line laid out in columns (an interlinear gloss line, a table): a run
/// of two spaces or a tab inside it. Such a line never joins another.
fn aligned(line: &str) -> bool {
    let t = line.trim();
    t.contains("  ") || t.contains('\t')
}

fn has_final_mark(s: &str) -> bool {
    s.chars().any(|c| FINAL.contains(&c) || FINAL_TIGHT.contains(&c))
}

/// Joins wrapped lines: a space between them unless both sides are `tight`.
fn join_lines(run: &[&str]) -> String {
    let mut out = String::new();
    for line in run {
        let line = line.trim();
        if let (Some(prev), Some(next)) = (out.chars().last(), line.chars().next()) {
            if !(tight(prev) && tight(next)) {
                out.push(' ');
            }
        }
        out.push_str(line);
    }
    collapse(&out)
}

/// The segments of a plain-text file, in order, none empty, and how it was cut.
pub fn segments(text: &str) -> (Vec<String>, Cut) {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    let paragraphs: Vec<&[&str]> = lines.split(|l| l.trim().is_empty()).filter(|p| !p.is_empty()).collect();
    // The wrap width: the longest line not far past the median, so one long
    // line (a title, an address) does not hide the wrapping of the rest.
    let mut lengths: Vec<usize> = lines.iter().map(|l| l.trim().chars().count()).filter(|n| *n > 0).collect();
    lengths.sort_unstable();
    let median = lengths.get(lengths.len() / 2).copied().unwrap_or(0);
    let width = lengths.iter().copied().filter(|n| *n * 10 <= median * 13).max().unwrap_or(0);
    let len = |l: &str| l.trim().chars().count();
    // Whether line `a` broke only because `b`'s first word would not fit.
    let wrapped_before = |a: &str, b: &str| {
        // The next word, and the space before it; a tight script needs no space.
        let next = match b.trim().chars().next() {
            Some(c) if tight(c) => 1,
            _ => 1 + b.split_whitespace().next().map_or(0, |w| w.chars().count()),
        };
        !ends_sentence(a) && !aligned(a) && !aligned(b) && !labelled(b) && !numbered(b) && len(a) <= width && len(a) + next > width
    };
    let (mut open, mut breaks, mut capitals, mut commas) = (0, 0, 0, 0);
    for p in &paragraphs {
        for w in p.windows(2) {
            if !ends_sentence(w[0]) {
                open += 1;
                breaks += usize::from(wrapped_before(w[0], w[1]));
                capitals += usize::from(w[1].trim().chars().next().is_some_and(char::is_uppercase));
                commas += usize::from(w[0].trim_end().ends_with([',', ';', ':']));
            }
        }
    }
    // Verse starts nearly every line with a capital, or ends most with a comma.
    let verse = open >= 3 && (capitals * 10 >= open * 8 || commas * 10 >= open * 6);
    let wrapped = width >= 20 && breaks >= 2 && breaks * 10 >= open * 6 && !verse;
    let mut units: Vec<String> = Vec::new();
    for p in &paragraphs {
        let mut run: Vec<&str> = Vec::new();
        for (i, line) in p.iter().enumerate() {
            run.push(line);
            let joins = wrapped && p.get(i + 1).is_some_and(|next| wrapped_before(line, next));
            if !joins {
                let joined = join_lines(&run);
                // A run with no sentence mark at all is lines, not a wrapped sentence.
                if run.len() > 1 && !has_final_mark(&joined) {
                    units.extend(run.iter().map(|l| collapse(l)));
                } else {
                    units.push(joined);
                }
                run.clear();
            }
        }
    }
    units.retain(|u| !u.is_empty());
    // An example with its translation, or a speaker's turn, is one segment.
    let whole = |u: &str| numbered(u) || labelled(u);
    let several = units.iter().filter(|u| !whole(u) && sentences(u).len() > 1).count();
    let inner_blank = paragraphs.len() > 1;
    if !wrapped && !inner_blank && several * 2 <= units.len() {
        return (units, Cut::ByLine);
    }
    (merge_bare_punctuation(units.iter().flat_map(|u| if whole(u) { vec![u.clone()] } else { sentences(u) }).collect()), Cut::BySentence)
}

/// An example number at the start of a line (`1.`, `12a.`, `(3)`, `b)`):
/// it stays with the sentence after it.
fn enumerator(piece: &str) -> bool {
    let p = piece.strip_prefix('(').unwrap_or(piece);
    let Some(body) = p.strip_suffix('.').or_else(|| p.strip_suffix(')')) else { return false };
    let digits = body.chars().take_while(char::is_ascii_digit).count();
    let rest: Vec<char> = body.chars().skip(digits).collect();
    (1..=3).contains(&digits) && rest.len() <= 1 && rest.iter().all(char::is_ascii_lowercase) || digits == 0 && rest.len() == 1 && rest[0].is_ascii_lowercase()
}

/// A piece with no letter or digit (a lone `...`) joins the one before it.
fn merge_bare_punctuation(pieces: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in pieces {
        let quoted = p.len() > 1 && matches!((p.chars().next(), p.chars().last()), (Some('\'' | '‘'), Some('\'' | '’')));
        let bare = !p.chars().any(char::is_alphanumeric) || quoted;
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
            let next_upper = chars[end..].iter().find(|c| !c.is_whitespace()).is_some_and(|c| c.is_uppercase());
            if c == '.' && abbreviation(&chars[start..i], next_upper) {
                i = end;
                continue;
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
    let mut merged: Vec<String> = Vec::new();
    let mut carry: Option<String> = None;
    for piece in out {
        let piece = match carry.take() {
            Some(c) => format!("{c} {piece}"),
            None => piece,
        };
        if merged.is_empty() && enumerator(&piece) {
            carry = Some(piece);
        } else {
            merged.push(piece);
        }
    }
    merged.extend(carry);
    merged
}

fn push(out: &mut Vec<String>, piece: &[char]) {
    let s: String = piece.iter().collect::<String>().trim().to_string();
    if !s.is_empty() {
        out.push(s);
    }
}

const ABBREVIATIONS: [&str; 26] = [
    "dr", "mr", "mrs", "ms", "prof", "st", "nr", "no", "ca", "usw", "bzw", "vgl", "cf", "al", "ex", "pp", "sect", "fig", "vs", "etc", "pos", "adj", "adv", "pron", "jr", "sr",
];

/// Whether the word just before a full stop marks an abbreviation, not a
/// sentence end: one letter, a listed abbreviation, or one or two digits
/// before a capitalised word (an ordinal: `3. Oktober`).
fn abbreviation(before: &[char], next_upper: bool) -> bool {
    let word: String = before.iter().rev().take_while(|c| !c.is_whitespace() && !matches!(c, '(' | '[' | '"' | '\'' | '«')).collect::<Vec<_>>().into_iter().rev().collect();
    let n = word.chars().count();
    (n == 1 && word.chars().all(char::is_alphabetic))
        || (next_upper && (1..=2).contains(&n) && word.chars().all(|c| c.is_ascii_digit()))
        || ABBREVIATIONS.contains(&word.to_lowercase().as_str())
}

/// A line that opens with a timestamp (`[00:01]`, `0:04`) or a speaker label (`A:`, `MARIA: `).
fn labelled(line: &str) -> bool {
    let t = line.trim_start();
    let t = t.strip_prefix('[').unwrap_or(t);
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if (1..=2).contains(&digits) && t[digits..].starts_with(':') && t[digits + 1..].chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }
    match t.split_once(':') {
        Some((label, rest)) => !label.is_empty() && label.chars().count() <= 12 && !label.contains(char::is_whitespace) && rest.starts_with(' '),
        None => false,
    }
}

/// Whether a line starts with an example number (`1.`, `(3)`, `12a.`).
fn numbered(line: &str) -> bool {
    line.split_whitespace().next().is_some_and(enumerator)
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
        // Indented paragraphs with no blank line: a short last line ends its paragraph.
        let text = "The fox came to the well and looked\ndown into the water. Then it went.\n    Next day the fox came back to the\nwell, and the water was gone.\n    So it left again\n";
        assert_eq!(
            cut(text),
            vec!["The fox came to the well and looked down into the water.", "Then it went.", "Next day the fox came back to the well, and the water was gone.", "So it left again"]
        );
        assert_eq!(cut("The fox came to the well and\nlooked down into the water for\na long time.\n"), vec!["The fox came to the well and looked down into the water for a long time."]);
    }

    #[test]
    fn linguists_files_keep_their_lines() {
        // Numbered examples keep their numbers.
        assert_eq!(segments("1. The dog ate the fish.\n2. The man saw the dog.\n(3) Who came?\n12a. Kesi mopa.\n"), (vec!["1. The dog ate the fish.".to_string(), "2. The man saw the dog.".into(), "(3) Who came?".into(), "12a. Kesi mopa.".into()], Cut::ByLine));
        // Unpunctuated lines of similar length, Chinese and Japanese utterances: never one segment.
        let text = "so we went down to the river that day\nand nobody there said a word to us\nthe old man kept on walking in front\nuntil we came to the place by the well\n";
        assert_eq!(cut(text).len(), 4);
        assert_eq!(cut("我们昨天去了河边看看那里的情况怎么样\n他说那里的水已经干了很久了不能再喝\n后来我们就回家吃饭了大家都很累\n").len(), 3);
        // Hard-wrapped Chinese joins with nothing between the characters.
        assert_eq!(cut("我们昨天去了河边，看了很久，然后我们回家\n吃饭了。他说河水已经干了，但是河水已经干\n了很久。\n"), vec!["我们昨天去了河边，看了很久，然后我们回家吃饭了。", "他说河水已经干了，但是河水已经干了很久。"]);
        // Interlinear lines in columns stay apart.
        let igt = "Kesi    mopa-n   toru\ndog     eat-PST  fish\n'The dog ate the fish.'\n";
        assert_eq!(cut(igt).len(), 3);
        // Verse keeps its lines even with commas at their ends.
        let verse = "Shall I compare thee to a summer's day,\nThou art more lovely and more temperate,\nRough winds do shake the darling buds of May,\nAnd summer's lease hath all too short a date.\n";
        assert_eq!(cut(verse).len(), 4);
        // One long line does not hide the wrapping of the others.
        let wrapped = "Source: https://example.org/archive/collection/item/0001/transcript-full.txt\n\nThe fox came to the well one morning and looked\ndown into the water for a long time, and then it\nturned away and went back across the field to\nits den under the old oak tree by the river.\n";
        assert_eq!(cut(wrapped).len(), 2, "{:?}", cut(wrapped));
    }

    #[test]
    fn abbreviations_labels_and_quotes() {
        assert_eq!(cut("Am 3. Oktober 1990 kam er. Im 19. Jahrhundert, z. B. bei Dr. Müller, ca. 300 Leute.\n\nx"), vec!["Am 3. Oktober 1990 kam er.", "Im 19. Jahrhundert, z. B. bei Dr. Müller, ca. 300 Leute.", "x"]);
        assert_eq!(cut("kesi n. Dog. Also a name.\n\nx"), vec!["kesi n. Dog.", "Also a name.", "x"]);
        assert_eq!(cut("(1) Kesi mopa-n toru. 'The dog ate the fish.'\n(2) Mopa toru. 'Eat fish.'\n"), vec!["(1) Kesi mopa-n toru. 'The dog ate the fish.'", "(2) Mopa toru. 'Eat fish.'"]);
        assert_eq!(cut("A: Yes. We went there.\nB: No. Not at all.\n"), vec!["A: Yes. We went there.", "B: No. Not at all."]);
        let ts = "[00:00:01] so we went down to the river with them all\n[00:00:04] and nobody was there when we got to it.\n[00:00:09] then we went back home again in the dark\n";
        assert_eq!(cut(ts).len(), 3);
        let verse = "the wind was cold upon the hill,\nthe river ran beneath the trees,\nand all the birds were singing still,\nupon the long and silver seas.\n";
        assert_eq!(cut(verse).len(), 4);
    }

    #[test]
    fn unpunctuated_lines_stay_lines() {
        assert_eq!(segments("kesi\nwiripa\ntela\nmanu\nnikani\n"), (vec!["kesi".to_string(), "wiripa".into(), "tela".into(), "manu".into(), "nikani".into()], Cut::ByLine));
        assert_eq!(cut("ah ya kesi mopa\nnikani ŋai toruma\nwiripa tela manu\nkesi kesi\n"), vec!["ah ya kesi mopa", "nikani ŋai toruma", "wiripa tela manu", "kesi kesi"]);
        // A transcript of long lowercase utterances is not mistaken for wrapping.
        let text = "so then we went down to the river with the others\nand nobody said anything\nbut the old man he just kept walking ahead of us all the way\nuntil we got there\n";
        assert_eq!(cut(text).len(), 4);
        // A paragraph per line is cut into sentences.
        assert_eq!(cut("One here. Two here.\nThree here. Four here.\n"), vec!["One here.", "Two here.", "Three here.", "Four here."]);
    }

    #[test]
    fn paragraphs_are_cut_into_sentences() {
        let text = "The fox came.  It drank from the well!\nDid it? \"Yes,\" she said. \"It did.\"\nThen, e.g. the cat came too.\n\nA new paragraph… With two sentences. ...";
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
