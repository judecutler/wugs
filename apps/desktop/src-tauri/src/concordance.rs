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

use wugs_core::Project;

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
    /// Words with a morph linked to this sense (the query is the sense's id): a lexicon entry's occurrences.
    Sense,
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

use wugs_model::{words_of, MARKERS};

/// A morph's surface without the edge markers some importers keep (FLEx `-ler`).
pub(crate) fn bare(surface: &str) -> &str {
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

/// The tiers the corpus tools read: (translation tiers, transcription tiers).
/// Translation: role `translation`, FLEx's free translation, or an ELAN
/// association tier with no role. Transcription: no role or `transcription`,
/// time-aligned or untyped. Live tiers only.
pub(crate) fn corpus_tiers(p: &Project) -> Result<(Vec<String>, Vec<String>), String> {
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
    Ok((translation_tiers, transcription_tiers))
}

/// One segment as the corpus tools read it.
pub(crate) struct CorpusSegment {
    pub text: String,
    pub title: String,
    pub id: String,
    pub tier: String,
    /// Its tokens (FLEx punctuation left out); empty for translations and
    /// segments not yet broken into words.
    pub tokens: Vec<String>,
    /// Its words: the tokens' forms, else the content split like the Gloss panel does.
    pub words: Vec<String>,
}

/// Words per segment on transcription tiers, for tests that count runs.
#[cfg(test)]
pub(crate) fn tests_word_counts(session: &mut Session) -> Vec<usize> {
    let p = project(session).unwrap();
    let mut out = Vec::new();
    each_segment(p, Read::Words, None, |seg| {
        out.push(seg.words.iter().filter(|w| !wugs_model::fold_key(&p.proj.nfc(w)).is_empty()).count());
        Ok(())
    })
    .unwrap();
    out
}

/// What a corpus reader needs from each segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Read {
    /// Transcription words, from the corpus index (no tokens).
    Words,
    /// Translation words, from the corpus index.
    Translations,
    /// Transcription words and the tokens behind them, for morphs and glosses.
    Analyses,
}

impl Read {
    pub(crate) fn of(kind: Kind) -> Read {
        match kind {
            Kind::Word => Read::Words,
            Kind::Translation => Read::Translations,
            Kind::Morph | Kind::Gloss | Kind::Sense => Read::Analyses,
        }
    }
}

/// Calls `visit` with every segment on a transcription tier (or, for
/// `Read::Translations`, a translation tier), text by text in order (text id,
/// then segment `pos`); `only_text` keeps one text. Words and translations
/// come from the projection's corpus index (`each_corpus_segment`), which
/// splits words the same way; `Read::Analyses` also reads each segment's
/// tokens from the entity tables.
pub(crate) fn each_segment(
    p: &Project,
    read: Read,
    only_text: Option<&str>,
    mut visit: impl FnMut(&CorpusSegment) -> Result<(), String>,
) -> Result<(), String> {
    let (translation_tiers, transcription_tiers) = corpus_tiers(p)?;
    let mut titles: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut title_of = |text: &str| -> Result<String, String> {
        if let Some(t) = titles.get(text) {
            return Ok(t.clone());
        }
        let t = p.proj.field_str(text, "title")?.unwrap_or_default();
        titles.insert(text.to_string(), t.clone());
        Ok(t)
    };
    if read != Read::Analyses {
        let tiers = if read == Read::Translations { &translation_tiers } else { &transcription_tiers };
        return p.proj.each_corpus_segment(tiers, only_text, read == Read::Translations, |seg, text, tier, words| {
            let title = title_of(text)?;
            visit(&CorpusSegment { text: text.to_string(), title, id: seg.to_string(), tier: tier.to_string(), tokens: Vec::new(), words: words.to_vec() })
        });
    }
    for text in p.proj.entities_of_kind("text")? {
        if only_text.is_some_and(|t| t != text) {
            continue;
        }
        let title = title_of(&text)?;
        for seg in p.proj.ordered_children("text", &text, "segment")? {
            let tier = p.proj.field_str(&seg, "tier")?.unwrap_or_default();
            if !transcription_tiers.contains(&tier) {
                continue;
            }
            let content = p.proj.field_str(&seg, "content")?.unwrap_or_default();
            let mut tokens: Vec<String> = p.proj.members(&seg)?;
            // FLEx keeps punctuation as tokens of their own; they are not words.
            tokens.retain(|t| p.proj.field_str(t, "x.flex.kind").ok().flatten().as_deref() != Some("punct"));
            let words: Vec<String> = if tokens.is_empty() {
                words_of(&content)
            } else {
                tokens.iter().map(|t| p.proj.field_str(t, "form").map(|f| f.unwrap_or_default())).collect::<Result<_, _>>()?
            };
            visit(&CorpusSegment { text: text.clone(), title: title.clone(), id: seg, tier, tokens, words })?;
        }
    }
    Ok(())
}

/// A morph as the corpus tools read it: surface and gloss without edge
/// markers' meaning lost (`type` says which marker), and its sense link.
pub(crate) struct CorpusMorph {
    pub surface: String,
    pub gloss: String,
    pub kind: String,
    pub sense: Option<String>,
}

pub(crate) fn morphs_of(p: &Project, token: &str) -> Result<Vec<CorpusMorph>, String> {
    let mut out = Vec::new();
    for m in p.proj.members(token)? {
        out.push(CorpusMorph {
            surface: p.proj.field_str(&m, "surface")?.unwrap_or_default(),
            gloss: p.proj.field_str(&m, "gloss")?.unwrap_or_default(),
            kind: p.proj.field_str(&m, "type")?.unwrap_or_else(|| "root".to_string()),
            sense: p.proj.field_str(&m, "sense")?,
        });
    }
    Ok(out)
}

pub fn concordance(session: &mut Session, query: &str, kind: Kind) -> Result<Concordance, String> {
    concordance_in(session, query, kind, Match::Glob, None)
}

/// How a query meets an item: as a pattern (`*`, `?`), or exactly (a row
/// clicked in Frequencies, whose spelling may hold a `?` glottal stop). An
/// exact query of several words meets them in a row, as Frequencies counts
/// n-grams (empty-key words skipped), and the hit is its first word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Match {
    Glob,
    Exact,
}

pub fn concordance_in(session: &mut Session, query: &str, kind: Kind, how: Match, only_text: Option<&str>) -> Result<Concordance, String> {
    let p = project(session)?;
    let meets = |pattern: &[char], item: &[char]| if how == Match::Exact { pattern == item } else { glob(pattern, item) };
    let key = |s: &str| wugs_model::fold_key(&p.proj.nfc(s)).chars().collect::<Vec<char>>();
    // Morphs and glosses meet without their edge markers, on both sides.
    let bare_key = |s: &str| wugs_model::lexicon_key(&p.proj.nfc(s)).chars().collect::<Vec<char>>();
    let pattern = match kind {
        Kind::Morph | Kind::Gloss => bare_key(query.trim()),
        Kind::Word | Kind::Translation => key(query.trim()),
        Kind::Sense => query.trim().chars().collect(),
    };
    let phrase: Vec<Vec<char>> = if how == Match::Exact && matches!(kind, Kind::Word | Kind::Translation) && query.split_whitespace().count() > 1 {
        query.split_whitespace().map(key).filter(|k| !k.is_empty()).collect()
    } else {
        Vec::new()
    };
    if pattern.is_empty() || (how == Match::Glob && pattern.iter().all(|c| *c == '*')) {
        return Ok(Concordance { hits: Vec::new(), total: 0 });
    }
    let mut hits = Vec::new();
    let mut total = 0usize;
    each_segment(p, Read::of(kind), only_text, |seg| {
        let mut found: Vec<(usize, Option<String>, Option<String>)> = Vec::new();
        match kind {
            Kind::Word | Kind::Translation if !phrase.is_empty() => {
                let words: Vec<(usize, Vec<char>)> = seg.words.iter().enumerate().map(|(i, w)| (i, key(w))).filter(|(_, k)| !k.is_empty()).collect();
                for run in words.windows(phrase.len()) {
                    if run.iter().zip(&phrase).all(|((_, k), q)| k == q) {
                        found.push((run[0].0, None, None));
                    }
                }
            }
            Kind::Word | Kind::Translation => {
                for (i, w) in seg.words.iter().enumerate() {
                    if meets(&pattern, &key(w)) {
                        found.push((i, None, None));
                    }
                }
            }
            Kind::Morph | Kind::Gloss | Kind::Sense => {
                for (i, token) in seg.tokens.iter().enumerate() {
                    let mut morphs = Vec::new();
                    let mut glosses = Vec::new();
                    let mut matched = false;
                    for m in morphs_of(p, token)? {
                        if kind == Kind::Sense {
                            matched |= m.sense.as_ref().is_some_and(|s| s.chars().eq(pattern.iter().copied()));
                        } else {
                            let probe = if kind == Kind::Morph { bare(&m.surface) } else { m.gloss.as_str() };
                            matched |= !probe.is_empty() && meets(&pattern, &bare_key(probe));
                        }
                        glosses.push((if m.gloss.is_empty() { "?".to_string() } else { bare(&m.gloss).to_string() }, m.kind.clone()));
                        morphs.push((bare(&m.surface).to_string(), m.kind));
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
                    text_id: seg.text.clone(),
                    text_title: seg.title.clone(),
                    segment_id: seg.id.clone(),
                    tier_id: seg.tier.clone(),
                    start_ms: p.proj.field_num(&seg.id, "start")?,
                    end_ms: p.proj.field_num(&seg.id, "end")?,
                    words: seg.words.clone(),
                    at,
                    breakdown,
                    glosses,
                });
            }
        }
        Ok(())
    })?;
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
        crate::sample::add_morphology(&mut session).unwrap();
        let w = concordance(&mut session, "FAɪV", Kind::Word).unwrap();
        assert_eq!(w.total, 1);
        assert_eq!(w.hits[0].words[w.hits[0].at], "faɪv");
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
        // two, three, to, ten
        assert_eq!(concordance(&mut session, "t*", Kind::Translation).unwrap().total, 4);
        assert_eq!(concordance(&mut session, "*", Kind::Word).unwrap().total, 0);
        // A lexicon entry's occurrences: the words whose morphs link to its sense.
        let p = project(&mut session).unwrap();
        let count = p.proj.entities_of_kind("sense").unwrap().into_iter().find(|s| p.proj.field_str(s, "gloss").unwrap().as_deref() == Some("count")).unwrap();
        let used = concordance(&mut session, &count, Kind::Sense).unwrap();
        assert_eq!(used.total, 1);
        assert_eq!(used.hits[0].breakdown.as_deref(), Some("ni-kani"));
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
