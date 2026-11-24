//! Words on the rise and falling over time (Scrolls, for dated corpora:
//! letters, newspapers, historical texts, recordings over decades): the texts
//! are grouped by the year a detail gives (a year, a decade or a century each,
//! as the Concordance's plot over time groups them), each word's rate per
//! 1000 words is worked out per period, and words whose rate goes steadily up
//! or down are listed by Kendall's tau between period and rate (Hilpert and
//! Gries 2009 argue for such rank-based trend tests on uneven diachronic data).

use std::collections::HashMap;

use serde::Serialize;

use crate::concordance::{each_segment, Kind, Read, Scope};
use crate::session::{project, Session};

/// Words listed each way.
pub const PER_SIDE: usize = 100;
/// How steady a trend must be to be listed.
pub const MIN_TAU: f64 = 0.5;
/// Periods with words needed before a trend means anything.
pub const MIN_PERIODS: usize = 3;

/// The year a detail's value names: its first run of 3 or 4 digits not part
/// of a longer number or a decimal, negative with BC or BCE after it. The
/// Concordance's `yearOf` reads it the same way.
pub fn year_of(value: &str) -> Option<i64> {
    let chars: Vec<char> = value.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        let before_ok = start == 0 || chars[start - 1] != '.';
        let after_ok = i == chars.len() || chars[i] != '.';
        let len = i - start;
        if (3..=4).contains(&len) && before_ok && after_ok {
            let year: i64 = chars[start..i].iter().collect::<String>().parse().ok()?;
            let rest: String = chars[i..].iter().collect::<String>().trim_start().to_uppercase();
            let bc = rest.starts_with("BCE") || rest.starts_with("BC") || rest.starts_with("B.C");
            return Some(if bc { -year } else { year });
        }
    }
    None
}

fn year_label(y: i64) -> String {
    if y < 0 {
        format!("{} BCE", -y)
    } else {
        y.to_string()
    }
}

/// Kendall's tau-b between two lists of numbers.
pub fn kendall(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    let (mut concordant, mut discordant, mut ties_x, mut ties_y) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        for j in i + 1..n {
            let dx = x[i] - x[j];
            let dy = y[i] - y[j];
            if dx == 0.0 && dy == 0.0 {
                continue;
            } else if dx == 0.0 {
                ties_x += 1.0;
            } else if dy == 0.0 {
                ties_y += 1.0;
            } else if (dx > 0.0) == (dy > 0.0) {
                concordant += 1.0;
            } else {
                discordant += 1.0;
            }
        }
    }
    let denom = ((concordant + discordant + ties_x) * (concordant + discordant + ties_y)).sqrt();
    if denom == 0.0 {
        0.0
    } else {
        (concordant - discordant) / denom
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trend {
    pub item: String,
    pub total: usize,
    /// Per 1000 words in each period; None where the period has no words.
    pub rates: Vec<Option<f64>>,
    pub tau: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trends {
    /// The details at least two texts give a year in, most first.
    pub keys: Vec<String>,
    /// The one read (empty when none can be).
    pub key: String,
    pub labels: Vec<String>,
    pub period_words: Vec<usize>,
    /// Texts in the scope with no year in that detail.
    pub undated: usize,
    /// Words used often enough to test.
    pub tested: usize,
    pub rising: Vec<Trend>,
    pub falling: Vec<Trend>,
}

pub fn trends(session: &mut Session, kind: Kind, key: Option<&str>, min_count: usize, scope: &Scope) -> Result<Trends, String> {
    if !matches!(kind, Kind::Word | Kind::Translation) {
        return Err("Trends are counted for words and translation words.".into());
    }
    let p = project(session)?;
    // Every live text's details, for the keys and the years.
    let mut details: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut with_year: HashMap<String, usize> = HashMap::new();
    for t in p.proj.entities_of_kind("text")? {
        if p.proj.is_tombstoned(&t)? {
            continue;
        }
        let meta: Vec<(String, String)> = p.text_metadata(&t)?.into_iter().collect();
        for (k, v) in &meta {
            if year_of(v).is_some() {
                *with_year.entry(k.clone()).or_default() += 1;
            }
        }
        details.insert(t, meta);
    }
    let mut keys: Vec<(String, usize)> = with_year.into_iter().filter(|(_, n)| *n >= 2).collect();
    keys.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let keys: Vec<String> = keys.into_iter().map(|(k, _)| k).collect();
    let key = key.filter(|k| keys.iter().any(|x| x == k)).map(str::to_string).or_else(|| keys.first().cloned()).unwrap_or_default();
    let mut out = Trends { keys, key: key.clone(), labels: Vec::new(), period_words: Vec::new(), undated: 0, tested: 0, rising: Vec::new(), falling: Vec::new() };
    if key.is_empty() {
        return Ok(out);
    }
    let years: HashMap<&str, i64> = details.iter().filter_map(|(t, meta)| meta.iter().find(|(k, _)| *k == key).and_then(|(_, v)| year_of(v)).map(|y| (t.as_str(), y))).collect();
    let (Some(lo), Some(hi)) = (years.values().min().copied(), years.values().max().copied()) else { return Ok(out) };
    let width = if hi - lo <= 30 { 1 } else if hi - lo <= 300 { 10 } else { 100 };
    let first = lo.div_euclid(width) * width;
    let periods = ((hi - first) / width + 1) as usize;
    out.labels = (0..periods)
        .map(|i| {
            let s = first + i as i64 * width;
            if width == 1 {
                year_label(s)
            } else if width == 10 && s >= 0 {
                format!("{s}s")
            } else {
                format!("{}–{}", year_label(s), year_label(s + width - 1))
            }
        })
        .collect();
    out.period_words = vec![0; periods];
    let mut counts: HashMap<String, (Vec<usize>, HashMap<String, usize>)> = HashMap::new();
    let mut undated = std::collections::HashSet::new();
    each_segment(p, Read::of(kind), scope, |seg| {
        let Some(year) = years.get(seg.text.as_str()) else {
            undated.insert(seg.text.clone());
            return Ok(());
        };
        let at = ((year - first) / width) as usize;
        for w in &seg.words {
            let spelling = p.proj.nfc(w);
            let k = wugs_model::fold_key(&spelling);
            if k.is_empty() {
                continue;
            }
            out.period_words[at] += 1;
            let entry = counts.entry(k).or_insert_with(|| (vec![0; periods], HashMap::new()));
            entry.0[at] += 1;
            *entry.1.entry(spelling).or_default() += 1;
        }
        Ok(())
    })?;
    out.undated = undated.len();
    let live: Vec<usize> = (0..periods).filter(|&i| out.period_words[i] > 0).collect();
    if live.len() < MIN_PERIODS {
        return Ok(out);
    }
    let xs: Vec<f64> = live.iter().map(|&i| i as f64).collect();
    let (mut rising, mut falling) = (Vec::new(), Vec::new());
    for (_, (per, spellings)) in counts {
        let total: usize = per.iter().sum();
        if total < min_count {
            continue;
        }
        out.tested += 1;
        let ys: Vec<f64> = live.iter().map(|&i| per[i] as f64 / out.period_words[i] as f64).collect();
        let tau = kendall(&xs, &ys);
        if tau.abs() < MIN_TAU {
            continue;
        }
        let item = spellings.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map(|(s, _)| s).unwrap_or_default();
        let rates = (0..periods).map(|i| (out.period_words[i] > 0).then(|| 1000.0 * per[i] as f64 / out.period_words[i] as f64)).collect();
        let trend = Trend { item, total, rates, tau };
        if tau > 0.0 {
            rising.push(trend);
        } else {
            falling.push(trend);
        }
    }
    let order = |a: &Trend, b: &Trend| b.tau.abs().total_cmp(&a.tau.abs()).then(b.total.cmp(&a.total)).then(a.item.cmp(&b.item));
    rising.sort_by(order);
    falling.sort_by(order);
    rising.truncate(PER_SIDE);
    falling.truncate(PER_SIDE);
    out.rising = rising;
    out.falling = falling;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn years_are_read_as_the_plot_reads_them() {
        assert_eq!(year_of("1953-04-02"), Some(1953));
        assert_eq!(year_of("c. 850"), Some(850));
        assert_eq!(year_of("450 BCE"), Some(-450));
        assert_eq!(year_of("12.345"), None);
        assert_eq!(year_of("no date"), None);
        assert_eq!(year_of("12345"), None);
    }

    #[test]
    fn kendall_is_one_for_a_steady_rise() {
        assert_eq!(kendall(&[0.0, 1.0, 2.0, 3.0], &[1.0, 2.0, 5.0, 9.0]), 1.0);
        assert_eq!(kendall(&[0.0, 1.0, 2.0], &[3.0, 2.0, 1.0]), -1.0);
        assert!(kendall(&[0.0, 1.0, 2.0, 3.0], &[1.0, 3.0, 2.0, 4.0]) > 0.0);
    }

    #[test]
    fn a_word_that_spreads_over_the_decades_rises() {
        let dir = std::env::temp_dir().join(format!("wugs-trends-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut session: Session = None;
        crate::session::create_project(&mut session, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let p = project(&mut session).unwrap();
        let tier = crate::session::utterance_tier(p).unwrap();
        // "okay" grows and "verily" fades across four decades; "the" stays.
        for (i, year) in ["1950", "1962", "1975", "1988"].iter().enumerate() {
            let mut lines = Vec::new();
            for n in 0..10 {
                let new = if n < i * 3 { "okay" } else { "the" };
                let old = if n < 9 - i * 3 { "verily" } else { "the" };
                lines.push(format!("the {new} {old} cat"));
            }
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            let (text, _) = p.import_written_text(&format!("Letters {year}"), &tier, &refs).unwrap();
            p.set_text_metadata(&text, "date", year).unwrap();
        }
        let (other, _) = p.import_written_text("Undated", &tier, &["the cat"]).unwrap();
        let _ = other;
        let t = trends(&mut session, Kind::Word, None, 5, &Scope::all()).unwrap();
        assert_eq!(t.key, "date");
        assert_eq!(t.labels, vec!["1950s", "1960s", "1970s", "1980s"]);
        assert_eq!(t.undated, 1);
        assert_eq!(t.rising.first().map(|r| r.item.as_str()), Some("okay"), "{:?}", t.rising);
        assert_eq!(t.falling.first().map(|r| r.item.as_str()), Some("verily"), "{:?}", t.falling);
        assert!(!t.rising.iter().chain(&t.falling).any(|r| r.item == "cat"));
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
