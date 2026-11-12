//! Concordance (roadmap 71): every occurrence of a word, morph, gloss or
//! translation word, each with the words of its segment so the panel can
//! show it in context (keyword in the middle) and sort by either side.
//!
//! Matching is exact up to case (NFC first, then lowercase): diacritics and
//! IPA marks are distinctions a linguist means, so `ŋa` never finds `na`.
//! `*` stands for any run of characters (`kan*`, `*ma`), `?` for one.

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

/// Words of a line: split on white space, punctuation at the edges left out,
/// the way the Gloss panel breaks a transcription into words.
pub fn words_of(line: &str) -> Vec<String> {
    line.split_whitespace()
        .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation() || "«»“”‘’„¿¡…".contains(c)).to_string())
        .filter(|w| !w.is_empty())
        .collect()
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
    let key = |s: &str| p.proj.nfc(s).to_lowercase().chars().collect::<Vec<char>>();
    let pattern = key(query.trim());
    if pattern.is_empty() || pattern.iter().all(|c| *c == '*') {
        return Ok(Concordance { hits: Vec::new(), total: 0 });
    }
    let translation_tiers: Vec<String> = p
        .proj
        .entities_of_kind("tier")?
        .into_iter()
        .filter(|t| p.proj.field_str(t, "role").ok().flatten().as_deref() == Some("translation"))
        .collect();
    let mut hits = Vec::new();
    let mut total = 0usize;
    for text in p.proj.entities_of_kind("text")? {
        let title = p.proj.field_str(&text, "title")?.unwrap_or_default();
        for seg in p.proj.ordered_children("text", &text, "segment")? {
            let tier = p.proj.field_str(&seg, "tier")?.unwrap_or_default();
            let is_translation = translation_tiers.contains(&tier);
            if is_translation != (kind == Kind::Translation) {
                continue;
            }
            let content = p.proj.field_str(&seg, "content")?.unwrap_or_default();
            let tokens: Vec<String> = if kind == Kind::Translation { Vec::new() } else { p.proj.members(&seg)? };
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
                            let probe = if kind == Kind::Morph { &surface } else { &gloss };
                            matched |= !probe.is_empty() && glob(&pattern, &key(probe));
                            glosses.push((if gloss.is_empty() { "?".to_string() } else { gloss }, kind_of.clone()));
                            morphs.push((surface, kind_of));
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
    fn words_leave_edge_punctuation_out() {
        assert_eq!(words_of("«Ŋai», toɾu… kesi."), vec!["Ŋai", "toɾu", "kesi"]);
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
        assert_eq!(m.hits[0].breakdown.as_deref(), Some("ni-kani"));
        assert_eq!(m.hits[0].glosses.as_deref(), Some("1SG-count"));
        assert_eq!(concordance(&mut session, "foc", Kind::Gloss).unwrap().total, 1);
        // two, three, ten
        assert_eq!(concordance(&mut session, "t*", Kind::Translation).unwrap().total, 3);
        assert_eq!(concordance(&mut session, "*", Kind::Word).unwrap().total, 0);
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
