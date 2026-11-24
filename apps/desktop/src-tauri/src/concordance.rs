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

use serde::{Deserialize, Serialize};

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
    /// Lemmas (CoNLL-U, or a glossed word's root entry): counted by Frequencies, found with a CQP query.
    Lemma,
    /// Part-of-speech tags (UPOS): counted by Frequencies, found with a CQP query.
    Tag,
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
    /// How many words the match covers from `at` (a phrase or a CQP query; else 1).
    pub len: usize,
    /// For a morph or gloss match: the word's morphs and their glosses, as the Gloss panel writes them.
    pub breakdown: Option<String>,
    pub glosses: Option<String>,
    /// For a CQP query: each word's tag (UPOS, else XPOS; empty when it has none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
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
pub(crate) fn joined(parts: &[(String, String)]) -> String {
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
    wugs_cldf::corpus_tiers(p)
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
    each_segment(p, Read::Words, &Scope::all(), |seg| {
        out.push(seg.words.iter().filter(|w| !wugs_model::fold_key(&p.proj.nfc(w)).is_empty()).count());
        Ok(())
    })
    .unwrap();
    out
}

/// A part of the corpus a tool may read: texts whose metadata `key` (made
/// with `metadata::key_of`) is exactly `value`, and/or only the segments of
/// one track (`tier`: a speaker's track, as CHAT files have one per
/// speaker). An empty key picks no texts by detail.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Where {
    pub key: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    /// `key` and `value` name a detail of the tracks' speakers (People
    /// panel) rather than of the texts: the tracks whose speaker has it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub speaker: bool,
}

impl Where {
    /// Texts whose detail `key` is `value`.
    pub fn detail(key: &str, value: &str) -> Where {
        Where { key: key.into(), value: value.into(), ..Where::default() }
    }
}

/// Which texts a corpus tool reads: one text, the texts a detail picks,
/// both together, or (neither) every text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    pub text: Option<String>,
    pub r#where: Option<Where>,
}

impl Scope {
    pub fn all() -> Scope {
        Scope::default()
    }

    pub fn text(id: &str) -> Scope {
        Scope { text: Some(id.to_string()), r#where: None }
    }

    pub fn of(text: Option<String>, r#where: Option<Where>) -> Scope {
        Scope { text: text.filter(|t| !t.is_empty()), r#where }
    }

    /// Whether it reads every text.
    pub fn is_all(&self) -> bool {
        self.text.is_none() && self.r#where.is_none()
    }

    /// The texts it keeps, worked out once: None keeps every text.
    /// The tracks it keeps: the one it names, or those whose speaker has
    /// the detail it names; None keeps every track.
    pub(crate) fn tiers(&self, p: &Project) -> Result<Option<std::collections::HashSet<String>>, String> {
        let Some(w) = self.r#where.as_ref() else { return Ok(None) };
        if let Some(t) = &w.tier {
            return Ok(Some(std::iter::once(t.clone()).collect()));
        }
        if !w.speaker {
            return Ok(None);
        }
        let key = wugs_core::metadata::key_of(&w.key).ok_or_else(|| format!("«{}» is not a detail's name", w.key))?;
        let value = p.proj.nfc(w.value.trim());
        let mut out = std::collections::HashSet::new();
        for t in p.proj.entities_of_kind("tier")? {
            let Some(person) = p.proj.field_text(&t, "speaker")? else { continue };
            if p.person_details(&person)?.iter().any(|(k, v)| *k == key && p.proj.nfc(v.trim()) == value) {
                out.insert(t);
            }
        }
        Ok(Some(out))
    }

    pub(crate) fn picked(&self, p: &Project) -> Result<Option<std::collections::HashSet<String>>, String> {
        let Some(w) = self.r#where.as_ref().filter(|w| !w.key.is_empty() && !w.speaker) else { return Ok(None) };
        let key = wugs_core::metadata::key_of(&w.key).ok_or_else(|| format!("«{}» is not a detail's name", w.key))?;
        let value = p.proj.nfc(w.value.trim());
        let mut out = std::collections::HashSet::new();
        for t in p.proj.entities_of_kind("text")? {
            if self.text.as_ref().is_some_and(|only| *only != t) {
                continue;
            }
            if p.proj.field_str(&t, &format!("{}{key}", wugs_core::metadata::PREFIX))?.map(|v| p.proj.nfc(&v)) == Some(value.clone()) {
                out.insert(t);
            }
        }
        Ok(Some(out))
    }

    /// Whether a segment of `text` on `tier` is inside, worked out once.
    pub(crate) fn member(&self, p: &Project) -> Result<Member, String> {
        Ok(Member { text: self.text.clone(), picked: self.picked(p)?, tiers: self.tiers(p)? })
    }
}

/// A scope's test for one segment (`Scope::member`).
pub(crate) struct Member {
    text: Option<String>,
    picked: Option<std::collections::HashSet<String>>,
    tiers: Option<std::collections::HashSet<String>>,
}

impl Member {
    pub(crate) fn has(&self, text: &str, tier: &str) -> bool {
        self.text.as_deref().is_none_or(|t| t == text) && self.picked.as_ref().is_none_or(|set| set.contains(text)) && self.tiers.as_ref().is_none_or(|set| set.contains(tier))
    }
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
            Kind::Morph | Kind::Gloss | Kind::Sense | Kind::Lemma | Kind::Tag => Read::Analyses,
        }
    }
}

/// Calls `visit` with every segment on a transcription tier (or, for
/// `Read::Translations`, a translation tier), text by text in order (text id,
/// then segment `pos`), within `scope`. Words and translations
/// come from the projection's corpus index (`each_corpus_segment`), which
/// splits words the same way; `Read::Analyses` also reads each segment's
/// tokens from the entity tables.
pub(crate) fn each_segment(
    p: &Project,
    read: Read,
    scope: &Scope,
    mut visit: impl FnMut(&CorpusSegment) -> Result<(), String>,
) -> Result<(), String> {
    let only_text = scope.text.as_deref();
    let picked = scope.picked(p)?;
    let keeps = |text: &str| picked.as_ref().is_none_or(|set| set.contains(text));
    let (mut translation_tiers, mut transcription_tiers) = corpus_tiers(p)?;
    if let Some(only) = scope.tiers(p)? {
        translation_tiers.retain(|t| only.contains(t));
        transcription_tiers.retain(|t| only.contains(t));
    }
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
            if !keeps(text) {
                return Ok(());
            }
            let title = title_of(text)?;
            visit(&CorpusSegment { text: text.to_string(), title, id: seg.to_string(), tier: tier.to_string(), tokens: Vec::new(), words: words.to_vec() })
        });
    }
    for text in p.proj.entities_of_kind("text")? {
        if only_text.is_some_and(|t| t != text) || !keeps(&text) {
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
            tokens.retain(|t| !p.proj.is_punct_token(t));
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

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub name: String,
}

/// The live transcription tracks, by name: what a corpus tool can be
/// narrowed to (one speaker's track).
/// The details of the people who speak on transcription tracks, each value
/// with how many tracks it picks: for the scope pickers' "Speakers by detail".
pub fn speaker_values(session: &mut Session) -> Result<Vec<(String, Vec<(String, usize)>)>, String> {
    let p = project(session)?;
    let (_, transcription) = corpus_tiers(p)?;
    let mut by_key: std::collections::BTreeMap<String, std::collections::BTreeMap<String, usize>> = Default::default();
    for t in transcription {
        let Some(person) = p.proj.field_text(&t, "speaker")? else { continue };
        if p.proj.is_tombstoned(&t)? || p.proj.is_tombstoned(&person)? {
            continue;
        }
        for (k, v) in p.person_details(&person)? {
            let v = v.trim().to_string();
            if !v.is_empty() {
                *by_key.entry(k).or_default().entry(v).or_default() += 1;
            }
        }
    }
    Ok(by_key
        .into_iter()
        .map(|(k, vals)| {
            let mut v: Vec<(String, usize)> = vals.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            (k, v)
        })
        .collect())
}

pub fn tracks(session: &mut Session) -> Result<Vec<Track>, String> {
    let p = project(session)?;
    let (_, transcription) = corpus_tiers(p)?;
    let mut out = Vec::new();
    for t in transcription {
        if p.proj.is_tombstoned(&t)? {
            continue;
        }
        out.push(Track { name: p.proj.field_str(&t, "name")?.unwrap_or_default(), id: t });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
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
    concordance_in(session, query, kind, Match::Glob, &Scope::all())
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

/// One match in a segment: the word it starts at, how many words it covers,
/// and for morphs and glosses the word's breakdown and glosses.
pub(crate) struct Found {
    pub at: usize,
    pub len: usize,
    pub breakdown: Option<String>,
    pub glosses: Option<String>,
}

/// Calls `on` with every segment in `scope` the kind reads, in corpus order,
/// and the matches of `query` in it (none for most segments), so the
/// Concordance and the dispersion plot meet items by one rule.
pub(crate) fn scan(
    p: &Project,
    query: &str,
    kind: Kind,
    how: Match,
    scope: &Scope,
    mut on: impl FnMut(&CorpusSegment, Vec<Found>) -> Result<(), String>,
) -> Result<(), String> {
    let meets = |pattern: &[char], item: &[char]| if how == Match::Exact { pattern == item } else { glob(pattern, item) };
    let key = |s: &str| wugs_model::fold_key(&p.proj.nfc(s)).chars().collect::<Vec<char>>();
    // Morphs and glosses meet without their edge markers, on both sides.
    let bare_key = |s: &str| wugs_model::lexicon_key(&p.proj.nfc(s)).chars().collect::<Vec<char>>();
    let pattern = match kind {
        Kind::Morph | Kind::Gloss => bare_key(query.trim()),
        Kind::Word | Kind::Translation => key(query.trim()),
        Kind::Sense => query.trim().chars().collect(),
        Kind::Lemma | Kind::Tag => return Err("lemmas and tags are found with a CQP query".into()),
    };
    let phrase: Vec<Vec<char>> = if how == Match::Exact && matches!(kind, Kind::Word | Kind::Translation) && query.split_whitespace().count() > 1 {
        query.split_whitespace().map(key).filter(|k| !k.is_empty()).collect()
    } else {
        Vec::new()
    };
    if pattern.is_empty() || (how == Match::Glob && pattern.iter().all(|c| *c == '*')) {
        return Ok(());
    }
    let len = if phrase.is_empty() { 1 } else { phrase.len() };
    each_segment(p, Read::of(kind), scope, |seg| {
        let mut found: Vec<Found> = Vec::new();
        let hit = |at: usize| Found { at, len, breakdown: None, glosses: None };
        match kind {
            Kind::Word | Kind::Translation if !phrase.is_empty() => {
                let words: Vec<(usize, Vec<char>)> = seg.words.iter().enumerate().map(|(i, w)| (i, key(w))).filter(|(_, k)| !k.is_empty()).collect();
                for run in words.windows(phrase.len()) {
                    if run.iter().zip(&phrase).all(|((_, k), q)| k == q) {
                        found.push(hit(run[0].0));
                    }
                }
            }
            Kind::Word | Kind::Translation => {
                for (i, w) in seg.words.iter().enumerate() {
                    if meets(&pattern, &key(w)) {
                        found.push(hit(i));
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
                        found.push(Found { at: i, len: 1, breakdown: Some(joined(&morphs)), glosses: Some(joined(&glosses)) });
                    }
                }
            }
            Kind::Lemma | Kind::Tag => unreachable!("refused above"),
        }
        on(seg, found)
    })
}

pub fn concordance_in(session: &mut Session, query: &str, kind: Kind, how: Match, scope: &Scope) -> Result<Concordance, String> {
    let p = project(session)?;
    let mut hits = Vec::new();
    let mut total = 0usize;
    scan(p, query, kind, how, scope, |seg, found| {
        for f in found {
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
                    at: f.at,
                    len: f.len,
                    breakdown: f.breakdown,
                    glosses: f.glosses,
                    tags: None,
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

    #[test]
    fn a_speakers_detail_picks_their_tracks() {
        let dir = std::env::temp_dir().join(format!("wugs-cc-speaker-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        let p = project(&mut s).unwrap();
        let person = p.add_named_person("Jude", &[]).unwrap();
        p.set_person_detail(&person, "sex", "female").unwrap();
        let utt = crate::session::utterance_tier(p).unwrap();
        p.set_tier_speaker(&utt, Some(&person)).unwrap();
        let count = |s: &mut Session, value: &str| {
            let scope = Scope::of(None, Some(Where { key: "sex".into(), value: value.into(), speaker: true, ..Where::default() }));
            let mut n = 0;
            each_segment(project(s).unwrap(), Read::Words, &scope, |seg| {
                n += seg.words.len();
                Ok(())
            })
            .unwrap();
            n
        };
        assert!(count(&mut s, "female") > 0, "her track's words");
        assert_eq!(count(&mut s, "male"), 0, "no male speaker");
        assert_eq!(speaker_values(&mut s).unwrap(), vec![("sex".to_string(), vec![("female".to_string(), 1)])]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
