//! The character inventory (the Checks panel): every character the
//! project's transcriptions and lexicon use, how often, and where one
//! occurs, as Paratext's and FLEx's character inventories show it. A
//! fieldworker's typos mostly live here: a Cyrillic «а» among Latin
//! letters, an ASCII apostrophe beside the glottal stop's «ʼ», a «g» beside
//! IPA «ɡ», a no-break space from a pasted word. Nothing here writes.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;
use unicode_normalization::{char::is_combining_mark, is_nfc};
use unicode_script::{Script, UnicodeScript};

use crate::session::{self, Session};

/// Where a character first occurs in a text.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    pub text_id: String,
    pub text_title: String,
    pub tier_id: String,
    pub segment_id: String,
    /// The segment's transcription, cut to about 60 characters around the character.
    pub context: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharRow {
    pub ch: String,
    /// `U+0251`.
    pub code: String,
    /// The Unicode name in lower case, or the code when it has none.
    pub name: String,
    pub script: String,
    /// `letter`, `mark`, `digit`, `space`, `invisible` or `other` (punctuation and symbols).
    pub kind: &'static str,
    pub in_texts: usize,
    pub in_lexicon: usize,
    pub place: Option<Place>,
    /// A lexicon entry using it, when no text does.
    pub entry: Option<(String, String)>,
    /// Why it may be a slip, in words; empty when nothing stands out.
    pub flags: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub rows: Vec<CharRow>,
    /// The script most letters are in.
    pub main_script: String,
    /// Transcriptions and lemmas not stored in NFC (from an older log or an import).
    pub not_nfc: usize,
}

/// Characters that are told apart in writing but look alike, so a project
/// using both is often using one by mistake. Each group is listed whole.
const LOOK_ALIKES: &[&[char]] = &[
    &['\'', '\u{2019}', '\u{02BC}', '\u{02BB}', '\u{2018}', '\u{00B4}', '`', '\u{02B9}'],
    &[':', '\u{02D0}', '\u{A789}'],
    &['g', '\u{0261}'],
    &['?', '\u{0294}', '\u{0242}'],
    &['\u{025B}', '\u{03B5}'],
    &['\u{0278}', '\u{03C6}'],
    &['\u{03B2}', '\u{A7B5}'],
    &['\u{03B8}', '\u{0473}'],
    &['\u{03C7}', 'x'],
    &['\u{0251}', '\u{03B1}', 'a'],
    &['\u{0254}', '\u{0186}'],
    &['-', '\u{2010}', '\u{2011}', '\u{2013}', '\u{2212}'],
];

/// Characters nobody sees on screen: they make two spellings that look the same differ.
fn invisible(c: char) -> bool {
    matches!(c, '\u{00A0}' | '\u{00AD}' | '\u{202F}' | '\u{2007}' | '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{FEFF}' | '\u{034F}') || (c.is_control() && c != '\n')
}

fn kind_of(c: char) -> &'static str {
    if invisible(c) {
        "invisible"
    } else if c.is_whitespace() {
        "space"
    } else if is_combining_mark(c) {
        "mark"
    } else if c.is_alphabetic() {
        "letter"
    } else if c.is_numeric() {
        "digit"
    } else {
        "other"
    }
}

fn script_name(s: Script) -> String {
    match s {
        Script::Common => "Common".into(),
        Script::Inherited => "Inherited".into(),
        other => other.full_name().to_string(),
    }
}

/// A short stretch of `text` around the first `c`, so the row shows where it is.
fn context(text: &str, c: char) -> String {
    let chars: Vec<char> = text.chars().collect();
    let at = chars.iter().position(|&x| x == c).unwrap_or(0);
    let from = at.saturating_sub(30);
    let to = (at + 30).min(chars.len());
    let mut out: String = chars[from..to].iter().collect();
    if from > 0 {
        out.insert(0, '…');
    }
    if to < chars.len() {
        out.push('…');
    }
    out
}

#[derive(Default)]
struct Count {
    texts: usize,
    lexicon: usize,
    place: Option<Place>,
    entry: Option<(String, String)>,
}

pub fn inventory(session: &mut Session) -> Result<Inventory, String> {
    let p = session::project(session)?;
    let (_, transcription) = crate::concordance::corpus_tiers(p)?;
    let mut counts: HashMap<char, Count> = HashMap::new();
    let mut not_nfc = 0;
    for text in p.proj.entities_of_kind("text")? {
        if p.proj.is_tombstoned(&text)? {
            continue;
        }
        let title = p.proj.field_str(&text, "title")?.unwrap_or_default();
        for seg in p.proj.ordered_children("text", &text, "segment")? {
            let tier = p.proj.field_str(&seg, "tier")?.unwrap_or_default();
            if !transcription.contains(&tier) || p.proj.is_tombstoned(&seg)? {
                continue;
            }
            let content = p.proj.field_str(&seg, "content")?.unwrap_or_default();
            if !is_nfc(&content) {
                not_nfc += 1;
            }
            for c in content.chars().filter(|&c| c != '\n') {
                let n = counts.entry(c).or_default();
                n.texts += 1;
                if n.place.is_none() {
                    n.place = Some(Place { text_id: text.clone(), text_title: title.clone(), tier_id: tier.clone(), segment_id: seg.clone(), context: context(&content, c) });
                }
            }
        }
    }
    for lex in p.proj.entities_of_kind("lexeme")? {
        if p.proj.is_tombstoned(&lex)? {
            continue;
        }
        let lemma = p.proj.field_str(&lex, "lemma")?.unwrap_or_default();
        if !is_nfc(&lemma) {
            not_nfc += 1;
        }
        for c in lemma.chars() {
            let n = counts.entry(c).or_default();
            n.lexicon += 1;
            if n.entry.is_none() {
                n.entry = Some((lex.clone(), lemma.clone()));
            }
        }
    }
    // The main script: the one most letter uses are in.
    let mut by_script: BTreeMap<String, usize> = BTreeMap::new();
    let mut letters = 0;
    for (c, n) in &counts {
        if kind_of(*c) == "letter" {
            letters += n.texts + n.lexicon;
            let s = c.script();
            if !matches!(s, Script::Common | Script::Inherited) {
                *by_script.entry(script_name(s)).or_default() += n.texts + n.lexicon;
            }
        }
    }
    let main_script = by_script.iter().max_by_key(|(_, n)| **n).map(|(s, _)| s.clone()).unwrap_or_default();
    let mut rows: Vec<CharRow> = counts
        .into_iter()
        .map(|(c, n)| {
            let kind = kind_of(c);
            let script = script_name(c.script());
            let mut flags = Vec::new();
            if kind == "invisible" {
                flags.push("Invisible: it makes words that look the same spell differently.".to_string());
            }
            if kind == "letter" && !main_script.is_empty() && script != main_script && !matches!(c.script(), Script::Common | Script::Inherited) {
                flags.push(format!("A {script} letter among {main_script} ones: often typed by mistake for a look-alike."));
            }
            if (kind == "letter" || kind == "mark") && n.texts + n.lexicon <= 2 && letters >= 200 {
                flags.push(format!("Used only {}.", if n.texts + n.lexicon == 1 { "once" } else { "twice" }));
            }
            CharRow {
                ch: c.to_string(),
                code: format!("U+{:04X}", c as u32),
                name: unicode_names2::name(c).map(|n| n.to_string().to_lowercase()).unwrap_or_else(|| format!("U+{:04X}", c as u32)),
                script,
                kind,
                in_texts: n.texts,
                in_lexicon: n.lexicon,
                place: n.place,
                entry: if n.texts == 0 { n.entry } else { None },
                flags,
            }
        })
        .collect();
    // Look-alikes: flagged only when the project uses more than one of a group.
    let used: std::collections::HashSet<char> = rows.iter().filter_map(|r| r.ch.chars().next()).collect();
    for row in &mut rows {
        let c = row.ch.chars().next().unwrap_or(' ');
        for group in LOOK_ALIKES {
            if !group.contains(&c) {
                continue;
            }
            let others: Vec<String> = group.iter().filter(|&&o| o != c && used.contains(&o)).map(|o| format!("«{o}» ({})", unicode_names2::name(*o).map(|n| n.to_string().to_lowercase()).unwrap_or_default())).collect();
            if !others.is_empty() {
                row.flags.push(format!("Looks like {}, which the project also uses.", others.join(" and ")));
            }
        }
    }
    let order = |k: &str| match k {
        "letter" => 0,
        "mark" => 1,
        "digit" => 2,
        "other" => 3,
        "space" => 4,
        _ => 5,
    };
    rows.sort_by(|a, b| order(a.kind).cmp(&order(b.kind)).then((b.in_texts + b.in_lexicon).cmp(&(a.in_texts + a.in_lexicon))).then(a.ch.cmp(&b.ch)));
    Ok(Inventory { rows, main_script, not_nfc })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn look_alikes_other_scripts_and_invisible_characters_are_flagged() {
        let dir = std::env::temp_dir().join(format!("wugs-characters-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let text = session::create_text(&mut s, "words").unwrap();
        // «а» in «kаla» is Cyrillic; «ʼ» and «'» are both apostrophes; a no-break space hides in the last line.
        for line in ["kala tama", "kаla", "taʼa ta'a", "kala\u{00A0}tama", "ɡa ga"] {
            session::add_segment(&mut s, &text, None, None, None, line).unwrap();
        }
        crate::lexicon::lexicon_add(&mut s, "ŋata", "lake").unwrap();
        let inv = inventory(&mut s).unwrap();
        assert_eq!(inv.main_script, "Latin");
        let row = |c: &str| inv.rows.iter().find(|r| r.ch == c).unwrap_or_else(|| panic!("{c} missing"));
        let a = row("a");
        assert_eq!((a.kind, a.in_texts, a.code.as_str(), a.name.as_str()), ("letter", 15, "U+0061", "latin small letter a"));
        assert!(a.flags.is_empty(), "{:?}", a.flags);
        let cyr = row("а");
        assert!(cyr.flags.iter().any(|f| f.starts_with("A Cyrillic letter among Latin")), "{:?}", cyr.flags);
        assert_eq!(cyr.place.as_ref().map(|p| p.context.as_str()), Some("kаla"));
        assert!(row("ʼ").flags.iter().any(|f| f.contains("«'»")), "{:?}", row("ʼ").flags);
        assert!(row("ɡ").flags.iter().any(|f| f.contains("«g»")));
        let nbsp = row("\u{00A0}");
        assert_eq!(nbsp.kind, "invisible");
        assert!(!nbsp.flags.is_empty());
        // A character only in the lexicon points at its entry.
        let eng = row("ŋ");
        assert_eq!((eng.in_texts, eng.in_lexicon), (0, 1));
        assert_eq!(eng.entry.as_ref().map(|e| e.1.as_str()), Some("ŋata"));
        // Letters come first, then the rest.
        assert_eq!(inv.rows[0].kind, "letter");
        assert_eq!(inv.not_nfc, 0);
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
