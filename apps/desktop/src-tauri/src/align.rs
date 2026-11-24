//! Pair a text with another text that translates it, sentence by sentence
//! (Gale and Church 1993): the two texts' sentences are aligned by their
//! lengths in characters, allowing one sentence to match one, two, or none,
//! and each translation is written as a translation of the sentence it
//! matched (on a translation track paired with the sentence's own track), as
//! a table's translation column is. A translation that matched two sentences
//! goes on the first. The translation text itself is left as it is.

use serde::Serialize;

use crate::interchange::one_track;
use crate::session::{project, Session};

/// The most sentences either text may have.
pub const MAX_SENTENCES: usize = 3000;

/// One bead: source sentences (0–2) and the translation sentences (0–2) they match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bead {
    pub source: std::ops::Range<usize>,
    pub target: std::ops::Range<usize>,
}

/// The standard normal distribution's upper tail twice: P(|Z| ≥ z).
fn two_tailed(z: f64) -> f64 {
    // Abramowitz and Stegun 7.1.26, through erf.
    let x = z.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    (poly * (-x * x).exp()).clamp(1e-300, 1.0)
}

/// Gale and Church's cost of matching `a` source characters with `b` target characters.
fn match_cost(a: usize, b: usize, c: f64) -> f64 {
    let (a, b) = (a as f64, b as f64);
    let mean = (a + b / c) / 2.0;
    let delta = if mean > 0.0 { (b - a * c) / (mean * 6.8).sqrt() } else { 0.0 };
    -100.0 * two_tailed(delta).ln()
}

/// The alignment of two lists of sentence lengths, in order.
pub fn gale_church(source: &[usize], target: &[usize]) -> Vec<Bead> {
    let (n, m) = (source.len(), target.len());
    let total_s: usize = source.iter().sum();
    let total_t: usize = target.iter().sum();
    let c = if total_s == 0 || total_t == 0 { 1.0 } else { total_t as f64 / total_s as f64 };
    // (source sentences, target sentences, penalty for the kind of bead) as -100 ln P.
    const MOVES: [(usize, usize, f64); 6] = [(1, 1, 0.0), (1, 0, 450.0), (0, 1, 450.0), (2, 1, 230.0), (1, 2, 230.0), (2, 2, 440.0)];
    let w = m + 1;
    let mut cost = vec![f64::INFINITY; (n + 1) * w];
    let mut back = vec![0u8; (n + 1) * w];
    cost[0] = 0.0;
    for i in 0..=n {
        for j in 0..=m {
            if i == 0 && j == 0 {
                continue;
            }
            let mut best = f64::INFINITY;
            for (k, &(di, dj, penalty)) in MOVES.iter().enumerate() {
                if di > i || dj > j {
                    continue;
                }
                let before = cost[(i - di) * w + (j - dj)];
                if !before.is_finite() {
                    continue;
                }
                let a: usize = source[i - di..i].iter().sum();
                let b: usize = target[j - dj..j].iter().sum();
                let here = before + penalty + match_cost(a, b, c);
                if here < best {
                    best = here;
                    back[i * w + j] = k as u8;
                }
            }
            cost[i * w + j] = best;
        }
    }
    let mut beads = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        let (di, dj, _) = MOVES[back[i * w + j] as usize];
        beads.push(Bead { source: i - di..i, target: j - dj..j });
        i -= di;
        j -= dj;
    }
    beads.reverse();
    beads
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pair {
    /// The source sentence the translation goes on (the first of two), if any.
    pub segment: Option<String>,
    pub source: Vec<String>,
    pub translation: Vec<String>,
    /// The source sentence has a translation on the track already.
    pub has_one: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alignment {
    pub pairs: Vec<Pair>,
    pub sources: usize,
    pub translations: usize,
    /// Pairs of one sentence and one.
    pub one_to_one: usize,
    /// Translation sentences matched to no sentence.
    pub unmatched: usize,
    /// Pairs whose sentence already has a translation, left as they are.
    pub already: usize,
    /// Written by `write`.
    pub written: usize,
}

fn sentences(p: &wugs_core::Project, text: &str, what: &str) -> Result<Vec<(String, String)>, String> {
    let mut notes = Vec::new();
    let segs = one_track(p, text, &mut notes, what)?;
    let mut out = Vec::new();
    for s in segs {
        let content = p.proj.field_str(&s, "content")?.unwrap_or_default();
        if !content.trim().is_empty() {
            out.push((s, content.trim().to_string()));
        }
    }
    Ok(out)
}

fn align(session: &mut Session, source: &str, translation: &str, write: bool) -> Result<Alignment, String> {
    if source == translation {
        return Err("Choose another text: a text cannot translate itself.".into());
    }
    let p = project(session)?;
    for t in [source, translation] {
        if p.proj.entity_kind(t)?.as_deref() != Some("text") || p.proj.is_tombstoned(t)? {
            return Err(format!("no text {t}"));
        }
    }
    let src = sentences(p, source, "Pairing")?;
    let tgt = sentences(p, translation, "Pairing")?;
    if src.is_empty() || tgt.is_empty() {
        return Err("Both texts need transcribed sentences to be paired.".into());
    }
    if src.len() > MAX_SENTENCES || tgt.len() > MAX_SENTENCES {
        return Err(format!("Pairing takes texts of at most {MAX_SENTENCES} sentences; split a longer text first."));
    }
    let len = |v: &[(String, String)]| v.iter().map(|(_, t)| t.chars().filter(|c| !c.is_whitespace()).count().max(1)).collect::<Vec<_>>();
    let beads = gale_church(&len(&src), &len(&tgt));
    let tier = p.proj.field_str(&src[0].0, "tier")?.unwrap_or_default();
    let track = if write { Some(crate::session::translation_track(p, &tier, None)?) } else { crate::session::existing_translation_track(p, &tier)? };
    let mut out = Alignment { pairs: Vec::new(), sources: src.len(), translations: tgt.len(), one_to_one: 0, unmatched: 0, already: 0, written: 0 };
    // A translation's `pos` follows its sentence's place in the text.
    let order: std::collections::HashMap<String, usize> = p.proj.ordered_children("text", source, "segment")?.into_iter().enumerate().map(|(i, s)| (s, i)).collect();
    let mut rows = Vec::new();
    for b in &beads {
        let segment = src.get(b.source.start).filter(|_| !b.source.is_empty()).map(|(id, _)| id.clone());
        let mut has_one = false;
        if let (Some(seg), Some(track)) = (&segment, &track) {
            for other in p.proj.children_by_ref("parent", seg, "segment")? {
                if p.proj.field_str(&other, "tier")?.as_deref() == Some(track.as_str()) && !p.proj.is_tombstoned(&other)? {
                    has_one = true;
                }
            }
        }
        if b.source.len() == 1 && b.target.len() == 1 {
            out.one_to_one += 1;
        }
        if b.source.is_empty() {
            out.unmatched += b.target.len();
        } else if has_one && !b.target.is_empty() {
            out.already += 1;
        }
        let translation: Vec<String> = tgt[b.target.clone()].iter().map(|(_, t)| t.clone()).collect();
        if let (Some(seg), false, false) = (&segment, has_one, translation.is_empty()) {
            rows.push((seg.clone(), order.get(seg).copied().unwrap_or(0), translation.join(" ")));
        }
        out.pairs.push(Pair { segment, source: src[b.source.clone()].iter().map(|(_, t)| t.clone()).collect(), translation, has_one });
    }
    if write {
        let track = track.expect("made above");
        out.written = p.add_translations(source, &track, &rows)?.0;
    }
    Ok(out)
}

/// The pairs `write` would make, writing nothing.
pub fn preview(session: &mut Session, source: &str, translation: &str) -> Result<Alignment, String> {
    align(session, source, translation, false)
}

/// Writes each matched translation on the source sentence it matched.
pub fn write(session: &mut Session, source: &str, translation: &str) -> Result<Alignment, String> {
    align(session, source, translation, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_texts_pair_one_to_one() {
        let b = gale_church(&[10, 30, 20], &[12, 33, 19]);
        assert_eq!(b, vec![Bead { source: 0..1, target: 0..1 }, Bead { source: 1..2, target: 1..2 }, Bead { source: 2..3, target: 2..3 }]);
    }

    #[test]
    fn a_long_translation_of_two_short_sentences_takes_both() {
        // The translator joined the second and third sentences.
        let b = gale_church(&[40, 15, 18, 50], &[42, 35, 52]);
        assert_eq!(b, vec![Bead { source: 0..1, target: 0..1 }, Bead { source: 1..3, target: 1..2 }, Bead { source: 3..4, target: 2..3 }]);
    }

    #[test]
    fn a_language_that_writes_longer_still_pairs() {
        // Every translation about twice as long: the ratio is learned from the totals.
        let b = gale_church(&[10, 25, 15, 30], &[21, 49, 31, 62]);
        assert_eq!(b.len(), 4);
        assert!(b.iter().all(|x| x.source.len() == 1 && x.target.len() == 1), "{b:?}");
    }

    #[test]
    fn two_written_texts_are_paired_and_written_once() {
        let dir = std::env::temp_dir().join(format!("wugs-align-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut session: Session = None;
        crate::session::create_project(&mut session, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let p = project(&mut session).unwrap();
        let tier = crate::session::utterance_tier(p).unwrap();
        let (src, _) = p.import_written_text("Story", &tier, &["Mtu mmoja alikwenda sokoni asubuhi na mapema.", "Alinunua ndizi.", "Kisha akarudi nyumbani kwa miguu, akiwa amechoka sana."]).unwrap();
        let (tgt, _) = p.import_written_text("Story (English)", &tier, &["One man went to the market early in the morning.", "He bought bananas.", "Then he walked back home, very tired."]).unwrap();
        let a = preview(&mut session, &src, &tgt).unwrap();
        assert_eq!((a.sources, a.translations, a.one_to_one, a.unmatched, a.written), (3, 3, 3, 0, 0));
        assert_eq!(a.pairs[1].translation, vec!["He bought bananas.".to_string()]);
        let w = write(&mut session, &src, &tgt).unwrap();
        assert_eq!(w.written, 3);
        let text = crate::session::get_text(&mut session, &src).unwrap();
        let first = text.segments.iter().find(|s| s.content.starts_with("Mtu")).unwrap().id.clone();
        let p = project(&mut session).unwrap();
        assert_eq!(crate::interchange::translations_of(p, &first).unwrap()[0].1, "One man went to the market early in the morning.");
        let again = write(&mut session, &src, &tgt).unwrap();
        assert_eq!((again.written, again.already), (0, 3));
        assert!(preview(&mut session, &src, &src).is_err());
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
