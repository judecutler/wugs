//! Concordance (roadmap 71): every occurrence of a word, morph, gloss or
//! translation word, each with the words of its segment so the panel can
//! show it in context (keyword in the middle) and sort by either side.
//!
//! Matching is exact up to case (NFC, then the app's `fold_key`, which also
//! folds Turkic dotted and dotless i): diacritics and IPA marks are
//! distinctions a linguist means, so `ŋa` never finds `na`. Morphs and glosses
//! compare without their edge markers (`lexicon_key`), so `-ra`, `ra` and a
//! FLEx morph stored as `-ra` all meet. `*` stands for any run of characters
//! (`kan*`, `*ma`), `?` for one.
//!
//! Which tiers: words, morphs and glosses come from transcription tiers;
//! translation words from translation tiers (role `translation`, FLEx's free
//! translation, or an ELAN association tier with no role). A segment whose
//! tier is gone is skipped.
//!
//! Cost: a query reads every segment, token and morph of the project on the
//! session thread (about a second per 20k words in a debug build); typed
//! tables (SCROLLS.md §2) are the fix once corpora grow.

use serde::Serialize;

use crate::session::{project, Session};

/// What to look for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Transcribed words (a segment's tokens, or its words when not yet broken out).
    Word,
    /// Morph surfaces.
    Morph,
    /// Morph glosses.
    Gloss,
    /// Words of free translations.
    Translation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub text_id: String,
    pub text_title: String,
    pub segment_id: String,
    pub tier_id: String,
    pub start_ms: Option<f64>,
    pub end_ms: Option<f64>,
    /// The segment's words, in order.
    pub words: Vec<String>,
    /// Which word holds the match.
    pub at: usize,
    /// For a morph or gloss match: the word's morphs and their glosses, as the Gloss panel writes them.
    pub breakdown: Option<String>,
    pub glosses: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Concordance {
    pub hits: Vec<Hit>,
    /// Every match found, even past the limit.
    pub total: usize,
}

/// At most this many hits are returned; `total` says how many there were.
pub const LIMIT: usize = 2000;

/// Glob matching on characters: `*` any run, `?` one.
pub fn glob(pattern: &[char], text: &[char]) -> bool {
    let (mut p, mut t) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            mark = t;
            p += 1;
        } else if let Some(s) = star {
            p = s + 1;
            mark += 1;
            t = mark;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

const MARKERS: [char; 5] = ['-', '=', '~', '<', '>'];

/// Combining marks and the spacing modifier letters IPA spells with (tone
/// letters, ʼ), which std's `is_alphanumeric` leaves out.
fn is_mark_or_modifier(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F | 0x02B0..=0x02FF | 0xA700..=0xA71F)
}
fn is_substance(c: char) -> bool {
    c.is_alphanumeric() || is_mark_or_modifier(c)
}
fn is_word_char(c: char) -> bool {
    is_substance(c) || MARKERS.contains(&c)
}
const APOSTROPHES: [char; 2] = ['\'', '\u{2019}'];

/// Words of a line, by the Gloss panel's rules (`src/gloss/tokenizer.ts`):
/// white space separates words; punctuation at a word's edge is left out but
/// an edge apostrophe stays (a glottal stop or ejective: `ka'`) unless the
/// word is wrapped in quotes; a chunk with no letter, mark or number in it is
/// punctuation alone and is not a word.
pub fn words_of(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in line.split(|c: char| c.is_whitespace()) {
        if !chunk.chars().any(is_substance) {
            continue;
        }
        let chars: Vec<char> = chunk.chars().collect();
        let (mut a, mut b) = (0, chars.len());
        let trim = |a: &mut usize, b: &mut usize| {
            while *a < *b && !is_word_char(chars[*a]) && !APOSTROPHES.contains(&chars[*a]) {
                *a += 1;
            }
            while *b > *a && !is_word_char(chars[*b - 1]) && !APOSTROPHES.contains(&chars[*b - 1]) {
                *b -= 1;
            }
        };
        trim(&mut a, &mut b);
        let opened_by_quote = a > 0 && chars[a - 1] == '\u{2018}';
        if opened_by_quote && b - a > 1 && APOSTROPHES.contains(&chars[b - 1]) {
            b -= 1;
            trim(&mut a, &mut b);
        } else if b - a > 2 && chars[a] == '\'' && APOSTROPHES.contains(&chars[b - 1]) {
            a += 1;
            b -= 1;
            trim(&mut a, &mut b);
        }
        let core: String = chars[a..b].iter().collect();
        if !core.is_empty() {
            out.push(core);
        }
    }
    out
}

/// A morph's surface without the edge markers some importers keep (FLEx `-ler`).
fn bare(surface: &str) -> &str {
    surface.trim_matches(|c| MARKERS.contains(&c))
}

const MARK: [(&str, &str, &str); 5] = [("prefix", "", "-"), ("suffix", "-", ""), ("infix", "<", ">"), ("clitic", "=", ""), ("reduplication", "", "~")];

/// Morphs joined at their markers, as the Leipzig word line writes them.
fn joined(parts: &[(String, String)]) -> String {
    let mut out = String::new();
    for (text, kind) in parts {
        let (before, after) = MARK.iter().find(|m| m.0 == kind).map(|m| (m.1, m.2)).unwrap_or(("", ""));
        let piece = format!("{before}{text}{after}");
        let glued = out.is_empty() || out.ends_with(['-', '=', '~', '>']) || piece.starts_with(['-', '=', '<']);
        if !glued {
            out.push(' ');
        }
        out.push_str(&piece);
    }
    out
}

pub fn concordance(session: &mut Session, query: &str, kind: Kind) -> Result<Concordance, String> {
    let p = project(session)?;
    let key = |s: &str| wugs_model::fold_key(&p.proj.nfc(s)).chars().collect::<Vec<char>>();
    // Morphs and glosses meet without their edge markers, on both sides.
    let bare_key = |s: &str| wugs_model::lexicon_key(&p.proj.nfc(s)).chars().collect::<Vec<char>>();
    let pattern = match kind {
        Kind::Morph | Kind::Gloss => bare_key(query.trim()),
        Kind::Word | Kind::Translation => key(query.trim()),
    };
    if pattern.is_empty() || pattern.iter().all(|c| *c == '*') {
        return Ok(Concordance { hits: Vec::new(), total: 0 });
    }
    // Live tiers only: a segment whose tier was deleted is skipped.
    let mut translation_tiers = Vec::new();
    let mut transcription_tiers = Vec::new();
    for t in p.proj.entities_of_kind("tier")? {
        let role = p.proj.field_str(&t, "role")?;
        let origin = p.proj.field_str(&t, "origin")?;
        let tier_kind = p.proj.field_str(&t, "kind")?;
        let translation = role.as_deref() == Some("translation")
            || origin.as_deref() == Some(wugs_flex::flextext::TIER_FREE)
            || (role.is_none() && tier_kind.as_deref() == Some("symbolic-association"));
        if translation {
            translation_tiers.push(t);
        } else if matches!(role.as_deref(), None | Some("transcription")) && matches!(tier_kind.as_deref(), None | Some("time-aligned")) {
            transcription_tiers.push(t);
        }
    }
    let mut hits = Vec::new();
    let mut total = 0usize;
    for text in p.proj.entities_of_kind("text")? {
        let title = p.proj.field_str(&text, "title")?.unwrap_or_default();
        for seg in p.proj.ordered_children("text", &text, "segment")? {
            let tier = p.proj.field_str(&seg, "tier")?.unwrap_or_default();
            let wanted = if kind == Kind::Translation { &translation_tiers } else { &transcription_tiers };
            if !wanted.contains(&tier) {
                continue;
            }
            let content = p.proj.field_str(&seg, "content")?.unwrap_or_default();
            let mut tokens: Vec<String> = if kind == Kind::Translation { Vec::new() } else { p.proj.members(&seg)? };
            // FLEx keeps punctuation as tokens of their own; they are not words.
            tokens.retain(|t| p.proj.field_str(t, "x.flex.kind").ok().flatten().as_deref() != Some("punct"));
            let words: Vec<String> = if tokens.is_empty() {
                words_of(&content)
            } else {
                tokens.iter().map(|t| p.proj.field_str(t, "form").map(|f| f.unwrap_or_default())).collect::<Result<_, _>>()?
            };
            let mut found: Vec<(usize, Option<String>, Option<String>)> = Vec::new();
            match kind {
                Kind::Word | Kind::Translation => {
                    for (i, w) in words.iter().enumerate() {
                        if glob(&pattern, &key(w)) {
                            found.push((i, None, None));
                        }
                    }
                }
                Kind::Morph | Kind::Gloss => {
                    for (i, token) in tokens.iter().enumerate() {
                        let mut morphs = Vec::new();
                        let mut glosses = Vec::new();
                        let mut matched = false;
                        for m in p.proj.members(token)? {
                            let surface = p.proj.field_str(&m, "surface")?.unwrap_or_default();
                            let gloss = p.proj.field_str(&m, "gloss")?.unwrap_or_default();
                            let kind_of = p.proj.field_str(&m, "type")?.unwrap_or_else(|| "root".to_string());
                            let probe = if kind == Kind::Morph { bare(&surface) } else { gloss.as_str() };
                            matched |= !probe.is_empty() && glob(&pattern, &bare_key(probe));
                            glosses.push((if gloss.is_empty() { "?".to_string() } else { bare(&gloss).to_string() }, kind_of.clone()));
                            morphs.push((bare(&surface).to_string(), kind_of));
                        }
                        if matched {
                            found.push((i, Some(joined(&morphs)), Some(joined(&glosses))));
                        }
                    }
                }
            }
            for (at, breakdown, glosses) in found {
                total += 1;
                if hits.len() < LIMIT {
                    hits.push(Hit {
                        text_id: text.clone(),
                        text_title: title.clone(),
                        segment_id: seg.clone(),
                        tier_id: tier.clone(),
                        start_ms: p.proj.field_num(&seg, "start")?,
                        end_ms: p.proj.field_num(&seg, "end")?,
                        words: words.clone(),
                        at,
                        breakdown,
                        glosses,
                    });
                }
            }
        }
    }
    Ok(Concordance { hits, total })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(p: &str, t: &str) -> bool {
        glob(&p.chars().collect::<Vec<_>>(), &t.chars().collect::<Vec<_>>())
    }

    #[test]
    fn globs_match_whole_words_with_wildcards() {
        assert!(g("kani", "kani"));
        assert!(!g("kani", "nikani"));
        assert!(g("*kani", "nikani"));
        assert!(g("ni*", "nikani"));
        assert!(g("n?kani", "nikani"));
        assert!(g("*a*", "ŋai"));
        assert!(!g("na", "ŋa"));
    }

    #[test]
    fn words_follow_the_gloss_panels_tokenizer() {
        assert_eq!(words_of("«Ŋai», toɾu… kesi."), vec!["Ŋai", "toɾu", "kesi"]);
        // An edge apostrophe is a glottal stop; a quoted word loses its quotes.
        assert_eq!(words_of("ka' 'ayo ‘word’"), vec!["ka'", "'ayo", "word"]);
        // Punctuation alone is not a word; Arabic and CJK punctuation is trimmed too.
        assert_eq!(words_of("— ... كتاب، 本。"), vec!["كتاب", "本"]);
        // Combining marks and tone letters spell words.
        assert_eq!(words_of("ma˥˩ a\u{0301}"), vec!["ma˥˩", "a\u{0301}"]);
    }

    #[test]
    fn morphs_join_at_their_markers() {
        let parts = |v: &[(&str, &str)]| v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>();
        assert_eq!(joined(&parts(&[("ni", "prefix"), ("kani", "root")])), "ni-kani");
        assert_eq!(joined(&parts(&[("teka", "root"), ("ma", "clitic")])), "teka=ma");
    }

    #[test]
    fn finds_words_morphs_glosses_and_translations_in_the_sample() {
        let dir = std::env::temp_dir().join(format!("wugs-conc-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        let w = concordance(&mut session, "KESI", Kind::Word).unwrap();
        assert_eq!(w.total, 1);
        assert_eq!(w.hits[0].words[w.hits[0].at], "kesi");
        let m = concordance(&mut session, "kani", Kind::Morph).unwrap();
        assert_eq!(m.total, 1);
        // With or without the marker, a morph meets.
        assert_eq!(concordance(&mut session, "ni-", Kind::Morph).unwrap().total, 1);
        assert_eq!(concordance(&mut session, "=ma", Kind::Morph).unwrap().total, 1);
        // Translations never show among words.
        assert_eq!(concordance(&mut session, "three", Kind::Word).unwrap().total, 0);
        assert_eq!(m.hits[0].breakdown.as_deref(), Some("ni-kani"));
        assert_eq!(m.hits[0].glosses.as_deref(), Some("1SG-count"));
        assert_eq!(concordance(&mut session, "foc", Kind::Gloss).unwrap().total, 1);
        // two, three, ten
        assert_eq!(concordance(&mut session, "t*", Kind::Translation).unwrap().total, 3);
        assert_eq!(concordance(&mut session, "*", Kind::Word).unwrap().total, 0);
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn turkic_i_folds_like_the_rest_of_the_app() {
        let g = |p: &str, t: &str| glob(&wugs_model::fold_key(p).chars().collect::<Vec<_>>(), &wugs_model::fold_key(t).chars().collect::<Vec<_>>());
        assert!(g("istanbul", "İstanbul"));
        assert!(g("ışık", "Işık"));
    }
}
