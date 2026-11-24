//! How alike the texts are in the words they use most: Cosine Delta (Smith
//! and Aldridge 2011; Evert et al. 2017), the measure stylometry uses to tell
//! authors, speakers, genres and dialects apart. The project's most frequent
//! words (`MFW`) are counted in each text as relative frequencies, each word
//! turned into z-scores across the texts, and two texts compared by the
//! cosine of their z-score vectors: 1 for texts that lean the same way on
//! every word, 0 for unrelated ones, below 0 for opposite habits. Needs three
//! texts or more; short texts (a few hundred words) give unsteady numbers.

use std::collections::HashMap;

use serde::Serialize;

use crate::concordance::{each_segment, Kind, Read, Scope};
use crate::session::{project, Session};

/// The most frequent words compared.
pub const MFW: usize = 200;
/// Texts compared at most: the longest ones.
pub const MAX_TEXTS: usize = 150;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextWords {
    pub id: String,
    pub title: String,
    /// Running words counted.
    pub words: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alike {
    /// The texts compared, in text order.
    pub texts: Vec<TextWords>,
    /// Words compared (at most `MFW`).
    pub mfw: usize,
    /// Cosine Delta similarity, `texts.len()` square; empty under three texts.
    pub similarity: Vec<Vec<f64>>,
    /// Texts with words left out past `MAX_TEXTS`.
    pub omitted: usize,
}

pub fn alike(session: &mut Session, kind: Kind) -> Result<Alike, String> {
    if !matches!(kind, Kind::Word | Kind::Translation) {
        return Err("texts are compared by their words or their translations' words".into());
    }
    let p = project(session)?;
    let mut order: Vec<String> = Vec::new();
    let mut per_text: HashMap<String, (String, HashMap<String, usize>, usize)> = HashMap::new();
    each_segment(p, Read::of(kind), &Scope::all(), |seg| {
        let entry = per_text.entry(seg.text.clone()).or_insert_with(|| {
            order.push(seg.text.clone());
            (seg.title.clone(), HashMap::new(), 0)
        });
        for w in &seg.words {
            let k = wugs_model::fold_key(&p.proj.nfc(w));
            if !k.is_empty() {
                *entry.1.entry(k).or_default() += 1;
                entry.2 += 1;
            }
        }
        Ok(())
    })?;
    order.retain(|t| per_text[t].2 > 0);
    let mut omitted = 0;
    if order.len() > MAX_TEXTS {
        let mut by_size = order.clone();
        by_size.sort_by(|a, b| per_text[b].2.cmp(&per_text[a].2).then(a.cmp(b)));
        let kept: std::collections::HashSet<String> = by_size.into_iter().take(MAX_TEXTS).collect();
        omitted = order.len() - MAX_TEXTS;
        order.retain(|t| kept.contains(t));
    }
    let mut totals: HashMap<&str, usize> = HashMap::new();
    for t in &order {
        for (k, n) in &per_text[t].1 {
            *totals.entry(k.as_str()).or_default() += n;
        }
    }
    let mut words: Vec<(&str, usize)> = totals.into_iter().collect();
    words.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    words.truncate(MFW);
    let texts: Vec<TextWords> = order.iter().map(|t| TextWords { id: t.clone(), title: per_text[t].0.clone(), words: per_text[t].2 }).collect();
    if order.len() < 3 {
        return Ok(Alike { texts, mfw: words.len(), similarity: Vec::new(), omitted });
    }
    let freq: Vec<Vec<f64>> = order
        .iter()
        .map(|t| {
            let (_, counts, n) = &per_text[t];
            words.iter().map(|(k, _)| *counts.get(*k).unwrap_or(&0) as f64 / *n as f64).collect()
        })
        .collect();
    Ok(Alike { texts, mfw: words.len(), similarity: cosine_delta(&freq), omitted })
}

/// Cosine Delta between rows of relative frequencies: each column z-scored
/// across rows (a column with no spread counts 0), then the cosine of every
/// two rows.
pub fn cosine_delta(freq: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let rows = freq.len();
    let cols = freq.first().map_or(0, Vec::len);
    let mut z = vec![vec![0.0; cols]; rows];
    for c in 0..cols {
        let mean = freq.iter().map(|r| r[c]).sum::<f64>() / rows as f64;
        let sd = (freq.iter().map(|r| (r[c] - mean).powi(2)).sum::<f64>() / rows as f64).sqrt();
        if sd > 0.0 {
            for r in 0..rows {
                z[r][c] = (freq[r][c] - mean) / sd;
            }
        }
    }
    let norm: Vec<f64> = z.iter().map(|r| r.iter().map(|x| x * x).sum::<f64>().sqrt()).collect();
    (0..rows)
        .map(|a| {
            (0..rows)
                .map(|b| {
                    if norm[a] == 0.0 || norm[b] == 0.0 {
                        return if a == b { 1.0 } else { 0.0 };
                    }
                    z[a].iter().zip(&z[b]).map(|(x, y)| x * y).sum::<f64>() / (norm[a] * norm[b])
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{self, Session};

    #[test]
    fn cosine_delta_of_three_rows() {
        // Two rows leaning the same way on both words, one the other way.
        let s = cosine_delta(&[vec![0.3, 0.1], vec![0.28, 0.12], vec![0.1, 0.3]]);
        assert!((s[0][0] - 1.0).abs() < 1e-9);
        assert!(s[0][1] > 0.9, "{s:?}");
        assert!(s[0][2] < -0.9, "{s:?}");
        assert!((s[1][2] - s[2][1]).abs() < 1e-12);
    }

    #[test]
    fn texts_in_the_same_words_come_out_alike() {
        let dir = std::env::temp_dir().join(format!("wugs-stylometry-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        let p = project(&mut s).unwrap();
        let tier = session::find_utterance_tier(p).unwrap().unwrap();
        p.import_written_text("Fish A", &tier, &["kuri mopa tela kuri", "kuri tela senu"]).unwrap();
        p.import_written_text("Fish B", &tier, &["kuri tela mopa", "kuri kuri senu tela"]).unwrap();
        p.import_written_text("Songs", &tier, &["waka lolo waka", "lolo waka pinu lolo"]).unwrap();
        let a = alike(&mut s, Kind::Word).unwrap();
        let at = |title: &str| a.texts.iter().position(|t| t.title == title).unwrap();
        let (fa, fb, so) = (at("Fish A"), at("Fish B"), at("Songs"));
        assert_eq!(a.texts.len(), 4, "the tutorial text too");
        assert!(a.similarity[fa][fb] > a.similarity[fa][so], "{:?}", a.similarity);
        assert!(a.similarity[fa][fb] > 0.5, "{:?}", a.similarity);
        assert_eq!(a.texts[fa].words, 7);
        assert!(alike(&mut s, Kind::Morph).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
