//! Glossed examples written as plain text: the numbered Leipzig examples of
//! papers, grammars, handouts and old Word files, copied into a `.txt`.
//!
//! ```text
//! (1) Ali   kitab-lar-ı   oku-du
//!     Ali   book-PL-ACC   read-PST
//!     'Ali read the books.'
//!
//! (2) Nikani ŋai
//!     ni-kani ŋai
//!     1SG-count one
//!     ‘I count one.’
//! ```
//!
//! Examples are separated by blank lines. In each, a closing line in quotes
//! is the free translation (anything after its closing quote, such as a
//! source in brackets, becomes a note); above it, the gloss line and the
//! morpheme line, whose words and pieces must match one for one (Leipzig
//! rules 1 and 2: pieces joined by `-`, `=` for clitics, `~` for
//! reduplication, `<…>` for an infix); above those, a line with as many
//! words as the morpheme line is the sentence as written (else the words are
//! the morpheme line without its markers); lines above that (a language
//! name, a source) become notes. An example number such as `(1)`, `12a.` or
//! `b.` at the start is kept as the sentence's reference.
//!
//! Each piece's type comes from where it stands: the root is the first piece
//! glossed with a lower-case word (a lexical gloss), else the first; pieces
//! before it are prefixes (`-`), proclitics (`=`) or reduplicants (`~`), and
//! after it suffixes, enclitics or reduplicants. Written as the Toolbox
//! interlinear import writes texts: one txn, roots linked to the one lexicon
//! sense with their form and gloss, a file imported twice matched.

use wugs_core::Project;

use crate::toolbox_text::{write_texts, Morph, Sentence, Text, TextReport, Writing};

const OPEN: [char; 5] = ['\'', '"', '‘', '“', '«'];

/// An example number at the start of a line: `(1)`, `(12a)`, `1.`, `12a.`, `a.`, `b)`.
fn example_number(line: &str) -> Option<(&str, &str)> {
    let t = line.trim_start();
    let (inner, rest) = if let Some(r) = t.strip_prefix('(') {
        let close = r.find(')')?;
        (&r[..close], &r[close + 1..])
    } else {
        let end = t.find(['.', ')'])?;
        (&t[..end], &t[end + 1..])
    };
    let digits = inner.trim_end_matches(|c: char| c.is_ascii_lowercase());
    let letters = &inner[digits.len()..];
    let ok = !inner.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
        && letters.len() <= 1
        && (!digits.is_empty() || letters.len() == 1)
        && digits.len() <= 4;
    (ok && rest.starts_with(char::is_whitespace)).then(|| (inner, rest.trim_start()))
}

/// The translation inside its quotes and whatever follows the closing quote.
fn translation(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    let open = t.chars().next().filter(|c| OPEN.contains(c))?;
    let close = match open {
        '‘' => '’',
        '“' => '”',
        '«' => '»',
        c => c,
    };
    let body = &t[open.len_utf8()..];
    // The last closing quote, so an apostrophe inside ('the dog's bone') stays.
    let end = body.rfind(close).or_else(|| (open == '‘').then(|| body.rfind('\'')).flatten())?;
    let inner = body[..end].trim().to_string();
    let after = body[end + close.len_utf8()..].trim().to_string();
    (!inner.is_empty()).then_some((inner, after))
}

/// A word split at its separators: pieces, and the separator after each (none after the last).
fn pieces(word: &str) -> (Vec<String>, Vec<char>) {
    let mut out = vec![String::new()];
    let mut seps = Vec::new();
    for c in word.chars() {
        if matches!(c, '-' | '=' | '~') {
            seps.push(c);
            out.push(String::new());
        } else {
            out.last_mut().unwrap().push(c);
        }
    }
    (out, seps)
}

/// `s<um>ulat` → (`sulat`, Some(`um`)).
fn infix(piece: &str) -> (String, Option<String>) {
    match (piece.find('<'), piece.find('>')) {
        (Some(a), Some(b)) if a < b => (format!("{}{}", &piece[..a], &piece[b + 1..]), Some(piece[a + 1..b].to_string())),
        _ => (piece.to_string(), None),
    }
}

/// A morpheme-line word and its gloss as morphs, or None when their pieces do not match.
fn morphs(word: &str, gloss: &str) -> Option<Vec<Morph>> {
    let (ws, wsep) = pieces(word);
    let (gs, gsep) = pieces(gloss);
    if ws.len() != gs.len() || ws.iter().any(String::is_empty) {
        return None;
    }
    // Separators must agree too, except a gloss may join with `-` where the word has `=` or `~`.
    if wsep.iter().zip(&gsep).any(|(a, b)| a != b && *b != '-') {
        return None;
    }
    let root = gs.iter().position(|g| infix(g).0.chars().any(char::is_lowercase)).unwrap_or(0);
    let mut out = Vec::new();
    for (i, (w, g)) in ws.iter().zip(&gs).enumerate() {
        let kind = if i == root {
            "root"
        } else {
            let sep = if i < root { wsep[i] } else { wsep[i - 1] };
            match (sep, i < root) {
                ('=', _) => "clitic",
                ('~', _) => "reduplication",
                (_, true) => "prefix",
                (_, false) => "suffix",
            }
        };
        let (surface, inner) = infix(w);
        let (gloss, inner_gloss) = infix(g);
        out.push(Morph { surface, kind, gloss, sense: None, ps: String::new() });
        if let Some(x) = inner {
            out.push(Morph { surface: x, kind: "infix", gloss: inner_gloss.unwrap_or_default(), sense: None, ps: String::new() });
        }
    }
    Some(out)
}

/// A word as written from its morpheme-line form: the markers taken out.
fn written(word: &str) -> String {
    word.chars().filter(|c| !matches!(c, '-' | '=' | '~' | '<' | '>')).collect()
}

/// Whether a gloss token holds a grammatical gloss: two capitals, or a person digit before capitals (`1SG`, `PST`, `3PL.ERG`).
fn grammatical(token: &str) -> bool {
    let caps: Vec<bool> = token.chars().map(|c| c.is_uppercase() || c.is_ascii_digit()).collect();
    caps.windows(2).any(|w| w[0] && w[1])
}

/// One example read from its block of lines, or None when it is not one.
fn example(block: &[&str]) -> Option<(Sentence, bool)> {
    let mut lines: Vec<String> = block.iter().map(|l| l.trim().to_string()).collect();
    let mut reference = String::new();
    if let Some((n, rest)) = example_number(&lines[0]) {
        reference = n.to_string();
        lines[0] = rest.to_string();
        if lines[0].is_empty() {
            lines.remove(0);
        }
    }
    let mut notes = Vec::new();
    let mut free = String::new();
    if let Some((t, after)) = lines.last().and_then(|l| translation(l)) {
        free = t;
        if !after.is_empty() {
            notes.push(after);
        }
        lines.pop();
    }
    if lines.len() < 2 {
        return None;
    }
    let gloss_line = lines.pop().unwrap();
    let morph_line = lines.pop().unwrap();
    let mwords: Vec<&str> = morph_line.split_whitespace().collect();
    let gwords: Vec<&str> = gloss_line.split_whitespace().collect();
    if mwords.is_empty() || mwords.len() != gwords.len() {
        return None;
    }
    let mut parsed = Vec::new();
    for (m, g) in mwords.iter().zip(&gwords) {
        parsed.push(morphs(m, g)?);
    }
    let surface: Vec<String> = match lines.last().map(|l| l.split_whitespace().map(str::to_string).collect::<Vec<_>>()) {
        Some(ws) if ws.len() == mwords.len() => {
            lines.pop();
            ws
        }
        _ => mwords.iter().map(|w| written(w)).collect(),
    };
    notes.splice(0..0, lines.into_iter().filter(|l| !l.is_empty()));
    let marked = mwords.iter().any(|w| w.contains(['-', '=', '~', '<'])) || gwords.iter().any(|g| grammatical(g));
    let words = surface.into_iter().zip(parsed).collect();
    Some((Sentence { reference, words, translation: free, notes, start: None, end: None }, marked))
}

fn blocks(input: &str) -> Vec<Vec<&str>> {
    let mut out: Vec<Vec<&str>> = vec![Vec::new()];
    for line in input.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            if !out.last().unwrap().is_empty() {
                out.push(Vec::new());
            }
        } else {
            out.last_mut().unwrap().push(line);
        }
    }
    out.retain(|b| !b.is_empty());
    out
}

/// The file's examples, and how many of its blocks were not examples.
fn read(input: &str) -> (Vec<Sentence>, usize, bool) {
    let mut sentences = Vec::new();
    let mut other = 0;
    let mut marked = false;
    for b in blocks(input.trim_start_matches('\u{feff}')) {
        match example(&b) {
            Some((s, m)) => {
                marked |= m;
                sentences.push(s);
            }
            None => other += 1,
        }
    }
    (sentences, other, marked)
}

/// Does this plain text hold glossed examples: at least one with a morpheme
/// boundary or a grammatical gloss, and at least two thirds of its blocks examples.
pub fn is_glossed_examples(input: &str) -> bool {
    let (sentences, other, marked) = read(input);
    marked && !sentences.is_empty() && sentences.len() * 3 >= (sentences.len() + other) * 2
}

/// Does any example carry a free translation (so the importer needs a translation track)?
pub fn has_translations(input: &str) -> bool {
    read(input).0.iter().any(|s| !s.translation.is_empty())
}

/// Import the examples as one text titled `title`, a sentence each, on
/// `tier`, translations on `translation_tier`. Blocks that are not examples
/// are counted under `not_carried` as `block`.
pub fn import_glossed_examples(p: &mut Project, input: &str, title: &str, tier: &str, translation_tier: Option<&str>) -> Result<TextReport, String> {
    let (sentences, other, _) = read(input);
    if sentences.is_empty() {
        return Err("no glossed examples were found (a word line and a gloss line whose words and pieces match)".into());
    }
    let mut report = TextReport::default();
    for _ in 0..other {
        report.not_carried.add("block");
    }
    write_texts(p, vec![Text { title: title.to_string(), sentences }], tier, translation_tier, Writing { intent: "import-glossed-examples", reference: "x.igt.number" }, &mut report)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "(1) Ali   kitab-lar-ı   oku-du\n    Ali   book-PL-ACC   read-PST\n    'Ali read the books.'\n\n(2) Turkish (made up)\n    Nikani ŋai\n    ni-kani ŋai\n    1SG-count one\n    ‘I count one.’ (Field notes, p. 3)\n\n3. s<um>ulat lo~lo=ma\n   <AV>write RED~go=Q\n   \"She writes; do they go?\"\n";

    #[test]
    fn numbers_and_translations_are_found() {
        assert_eq!(example_number("(12a) word"), Some(("12a", "word")));
        assert_eq!(example_number("b. word"), Some(("b", "word")));
        assert_eq!(example_number("3) word"), Some(("3", "word")));
        assert_eq!(example_number("Dr. Smith"), None);
        assert_eq!(example_number("1999. The year"), Some(("1999", "The year")));
        assert_eq!(translation("'the dog's bone'"), Some(("the dog's bone".into(), String::new())));
        assert_eq!(translation("‘I count one.’ (Field notes)"), Some(("I count one.".into(), "(Field notes)".into())));
        assert_eq!(translation("no quotes"), None);
    }

    #[test]
    fn pieces_get_their_types_from_the_root() {
        let m = morphs("ni-kani-ra=ma", "1SG-count-PST=Q").unwrap();
        let got: Vec<(&str, &str, &str)> = m.iter().map(|m| (m.surface.as_str(), m.kind, m.gloss.as_str())).collect();
        assert_eq!(got, vec![("ni", "prefix", "1SG"), ("kani", "root", "count"), ("ra", "suffix", "PST"), ("ma", "clitic", "Q")]);
        let m = morphs("s<um>ulat", "<AV>write").unwrap();
        assert_eq!(m.iter().map(|m| (m.surface.as_str(), m.kind, m.gloss.as_str())).collect::<Vec<_>>(), vec![("sulat", "root", "write"), ("um", "infix", "AV")]);
        assert_eq!(morphs("lo~lo", "RED~go").unwrap()[0].kind, "reduplication");
        assert!(morphs("kitab-lar", "book").is_none());
        assert!(morphs("a=b", "x.y-z").is_some(), "a gloss may join a clitic with -");
    }

    #[test]
    fn a_file_of_examples_is_read_whole() {
        assert!(is_glossed_examples(SAMPLE));
        assert!(has_translations(SAMPLE));
        let (s, other, _) = read(SAMPLE);
        assert_eq!(other, 0);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].reference, "1");
        assert_eq!(s[0].words.iter().map(|w| w.0.as_str()).collect::<Vec<_>>(), vec!["Ali", "kitabları", "okudu"]);
        assert_eq!(s[0].translation, "Ali read the books.");
        // A sentence line above the morphemes, a header and a source as notes.
        assert_eq!(s[1].words[0].0, "Nikani");
        assert_eq!(s[1].notes, vec!["Turkish (made up)".to_string(), "(Field notes, p. 3)".to_string()]);
        assert_eq!(s[2].words[0].0, "sumulat");
        assert_eq!(s[2].translation, "She writes; do they go?");
    }

    #[test]
    fn prose_is_not_examples() {
        assert!(!is_glossed_examples("It was a well-known story.\nShe told it every year.\n\nThe end came soon."));
        assert!(!is_glossed_examples("one two three\nfour five six\n"));
        // Two prose blocks for one example are too many.
        assert!(!is_glossed_examples("Some prose here.\n\nMore prose there.\n\n(1) a-b\n    x-PL\n"));
    }
}
