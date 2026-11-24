//! Likely meanings of a word from the translations: for each word form, the
//! translation words that stand in the same sentences' translations most
//! often, scored by Dice's coefficient (twice the sentences with both, over
//! the sentences with the word plus those whose translation has the
//! translation word), the measure word aligners start from. A first guess at
//! a gloss in a language with translated sentences and no lexicon yet;
//! nothing is written.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use wugs_core::Project;
use wugs_model::fold_key;

use crate::concordance::{each_segment, Read, Scope};

/// Candidates kept per word.
pub const PER_WORD: usize = 3;
/// Sentences a word and a translation word must share.
pub const MIN_TOGETHER: usize = 2;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Meaning {
    /// The translation word, case folded.
    pub word: String,
    /// Translated sentences holding both.
    pub together: usize,
    /// Translated sentences holding the word form.
    pub of: usize,
    /// Dice's coefficient, 0 to 1.
    pub dice: f64,
}

/// For each form (as written, folded here), its likeliest translation words,
/// best first; an empty list when none is met in `MIN_TOGETHER` sentences.
pub fn likely_meanings(p: &Project, scope: &Scope, forms: &[String]) -> Result<Vec<Vec<Meaning>>, String> {
    let key = |s: &str| fold_key(&p.proj.nfc(s));
    let targets: HashMap<String, usize> = forms.iter().enumerate().map(|(i, f)| (key(f), i)).collect();
    let mut sentences_with: Vec<usize> = vec![0; forms.len()];
    let mut translation_count: HashMap<String, usize> = HashMap::new();
    let mut together: Vec<HashMap<String, usize>> = vec![HashMap::new(); forms.len()];
    // Each sentence's translation, read once from the corpus index as
    // `translations_of` pairs them: the one naming it as `parent`, else the
    // timed one overlapping it most.
    let mut by_parent: HashMap<String, HashSet<String>> = HashMap::new();
    let mut timed: HashMap<String, Vec<(f64, f64, HashSet<String>)>> = HashMap::new();
    each_segment(p, Read::Translations, scope, |seg| {
        let tw: HashSet<String> = seg.words.iter().map(|w| key(w)).filter(|w| !w.is_empty()).collect();
        if tw.is_empty() {
            return Ok(());
        }
        if let Some(parent) = p.proj.field_str(&seg.id, "parent")? {
            by_parent.entry(parent).or_insert(tw);
        } else if let (Some(a), Some(b)) = (p.proj.field_num(&seg.id, "start")?, p.proj.field_num(&seg.id, "end")?) {
            timed.entry(seg.text.clone()).or_default().push((a, b, tw));
        }
        Ok(())
    })?;
    each_segment(p, Read::Words, scope, |seg| {
        let here: HashSet<usize> = seg.words.iter().filter_map(|w| targets.get(&key(w)).copied()).collect();
        // Every translated sentence counts toward a translation word's total,
        // even one with no listed form, so common words score low.
        let tw = match by_parent.get(&seg.id) {
            Some(tw) => tw,
            None => {
                let Some(spans) = timed.get(&seg.text) else { return Ok(()) };
                let (Some(start), Some(end)) = (p.proj.field_num(&seg.id, "start")?, p.proj.field_num(&seg.id, "end")?) else { return Ok(()) };
                let best = spans
                    .iter()
                    .map(|(a, b, tw)| (end.min(*b) - start.max(*a), tw))
                    .filter(|(o, _)| *o > 0.0)
                    .max_by(|x, y| x.0.total_cmp(&y.0));
                let Some((_, tw)) = best else { return Ok(()) };
                tw
            }
        };
        for t in tw {
            *translation_count.entry(t.clone()).or_default() += 1;
        }
        for &i in &here {
            sentences_with[i] += 1;
            for t in tw {
                *together[i].entry(t.clone()).or_default() += 1;
            }
        }
        Ok(())
    })?;
    Ok(together
        .into_iter()
        .enumerate()
        .map(|(i, pairs)| {
            let of = sentences_with[i];
            let mut list: Vec<Meaning> = pairs
                .into_iter()
                .filter(|(_, n)| *n >= MIN_TOGETHER)
                .map(|(word, n)| {
                    let dice = 2.0 * n as f64 / (of + translation_count[&word]) as f64;
                    Meaning { word, together: n, of, dice }
                })
                .collect();
            list.sort_by(|a, b| b.dice.total_cmp(&a.dice).then(b.together.cmp(&a.together)).then(a.word.cmp(&b.word)));
            list.truncate(PER_WORD);
            list
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{self, project, Session};

    #[test]
    fn the_word_that_always_stands_with_it_wins() {
        let dir = std::env::temp_dir().join(format!("wugs-meanings-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        let p = project(&mut s).unwrap();
        let tier = session::find_utterance_tier(p).unwrap().unwrap();
        let trans = session::translation_track(p, &tier, None).unwrap();
        let rows = [
            ("kuri mopa", "the dog sleeps"),
            ("kuri tela", "the dog eats"),
            ("Kuri senu mopa", "the big dog sleeps"),
            ("waka tela", "the cat eats"),
            ("waka mopa", "the cat sleeps"),
        ];
        let written: Vec<wugs_core::WrittenRow> = rows.iter().map(|(v, t)| wugs_core::WrittenRow { text: v, translation: t, extras: Vec::new(), span: None, tier: None }).collect();
        p.import_written_rows("Animals", &tier, Some(&trans), &written).unwrap();
        let forms = vec!["kuri".to_string(), "mopa".to_string(), "senu".to_string()];
        let m = likely_meanings(p, &Scope::all(), &forms).unwrap();
        assert_eq!(m[0][0].word, "dog");
        assert_eq!((m[0][0].together, m[0][0].of), (3, 3));
        assert!((m[0][0].dice - 1.0).abs() < 1e-9);
        // "the" is in every translation (and the tutorial's), so it scores lower.
        let the = m[0].iter().find(|x| x.word == "the").unwrap();
        assert!(the.dice < 0.8, "{the:?}");
        assert_eq!(m[1][0].word, "sleeps");
        // Met once: no candidate reaches two sentences.
        assert!(m[2].is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
