//! Gloss by word type (FLEx's word analyses): every word form not yet broken
//! into morphs somewhere in the texts, commonest first, each analysed once for
//! all its unanalysed uses (`Project::apply_analysis_to_unanalysed`). Words
//! live on tokens, so sentences not yet broken into words can be broken all at
//! once first (`break_all`), as the Gloss panel's Break into words does one.

use serde::Serialize;
use wugs_core::{MorphSpec, Project};

use crate::concordance::{each_segment, Kind, Read, Scope};
use crate::frequencies::frequencies;
use crate::gloss::{MorphSpecView, SuggestionView};
use crate::session::{project, Session};

/// Word types listed with a suggestion each; the rest are listed bare.
const SUGGESTED: usize = 200;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeRow {
    /// The spelling met most often.
    pub form: String,
    /// Uses not yet broken into morphs.
    pub unanalysed: usize,
    /// All its uses in the texts.
    pub uses: usize,
    /// The commonest earlier analysis of the form, else a guess from known morphs.
    pub suggestion: Option<SuggestionView>,
    /// Translation words that stand with it in the same sentences' translations.
    pub meanings: Vec<crate::meanings::Meaning>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordTypes {
    /// Forms with unanalysed uses, most unanalysed uses first.
    pub rows: Vec<TypeRow>,
    /// Running words whose sentence is not yet broken into words (not listed).
    pub unbroken_words: usize,
    /// Those sentences.
    pub unbroken_sentences: usize,
    /// Every word form counted (the list stops at the frequency list's limit).
    pub types: usize,
}

pub fn word_types(session: &mut Session, text: Option<&str>) -> Result<WordTypes, String> {
    let scope = match text {
        Some(t) => Scope::text(t),
        None => Scope::all(),
    };
    let freq = frequencies(session, Kind::Word, &scope)?;
    let p = project(session)?;
    let mut unbroken_words = 0;
    let mut unbroken_sentences = 0;
    each_segment(p, Read::Analyses, &scope, |seg| {
        if seg.tokens.is_empty() && !seg.words.is_empty() {
            unbroken_sentences += 1;
            unbroken_words += seg.words.len();
        }
        Ok(())
    })?;
    let forms: Vec<String> = freq.rows.iter().map(|r| r.item.clone()).collect();
    let unanalysed = p.unanalysed_counts(&forms)?;
    let mut rows: Vec<TypeRow> = freq
        .rows
        .iter()
        .zip(unanalysed)
        .filter(|(_, n)| *n > 0)
        .map(|(r, n)| TypeRow { form: r.item.clone(), unanalysed: n, uses: r.count, suggestion: None, meanings: Vec::new() })
        .collect();
    // Stable: equal counts keep the frequency list's order.
    rows.sort_by(|a, b| b.unanalysed.cmp(&a.unanalysed));
    let inventory = crate::gloss::inventory(p)?;
    for row in rows.iter_mut().take(SUGGESTED) {
        let earlier = p.analysis_suggestions(&row.form)?;
        let (first, guessed) = match earlier.into_iter().next() {
            Some(s) => (Some(s), false),
            None => (p.guess_analysis(&inventory, &row.form).into_iter().next(), true),
        };
        row.suggestion = first.map(|s| SuggestionView { morphs: s.morphs.into_iter().map(Into::into).collect(), count: s.count, guessed });
    }
    let suggested: Vec<String> = rows.iter().take(SUGGESTED).map(|r| r.form.clone()).collect();
    for (row, meanings) in rows.iter_mut().zip(crate::meanings::likely_meanings(p, &scope, &suggested)?) {
        row.meanings = meanings;
    }
    Ok(WordTypes { rows, unbroken_words, unbroken_sentences, types: freq.types })
}

/// Break every sentence with text but no words yet into words, in one text
/// or all; how many sentences were broken.
pub fn break_all(session: &mut Session, text: Option<&str>) -> Result<usize, String> {
    let scope = match text {
        Some(t) => Scope::text(t),
        None => Scope::all(),
    };
    let p = project(session)?;
    let mut todo: Vec<(String, String, Vec<String>)> = Vec::new();
    each_segment(p, Read::Analyses, &scope, |seg| {
        if seg.tokens.is_empty() && !seg.words.is_empty() {
            todo.push((seg.text.clone(), seg.id.clone(), seg.words.clone()));
        }
        Ok(())
    })?;
    for (text, seg, words) in &todo {
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        p.tokenize(text, seg, &refs)?;
    }
    Ok(todo.len())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeApplied {
    pub applied: usize,
    pub texts: usize,
    /// Morphs of the analysis linked to a lexicon sense they name.
    pub linked: usize,
}

/// The one live sense whose entry is spelled like `surface` (markers aside)
/// and whose gloss is `gloss` (case-blind), if exactly one is.
fn sense_named(p: &Project, surface: &str, gloss: &str) -> Result<Option<String>, String> {
    let key = wugs_model::lexicon_key(&p.proj.nfc(surface));
    let want = wugs_model::fold_key(gloss.trim());
    let mut found: Vec<String> = Vec::new();
    for lex in p.proj.by_lookup_key("lexeme", &key)? {
        if p.proj.is_tombstoned(&lex)? {
            continue;
        }
        for sen in p.proj.ordered_children("lexeme", &lex, "sense")? {
            if wugs_model::fold_key(p.proj.field_str(&sen, "gloss")?.unwrap_or_default().trim()) == want {
                found.push(sen);
            }
        }
    }
    Ok(if found.len() == 1 { found.pop() } else { None })
}

/// One analysis for every unanalysed use of `form`. A morph typed with no
/// sense is linked to the lexicon when exactly one sense has its spelling
/// and gloss, as the Gloss panel's lexicon autocomplete would link it.
pub fn apply_type(session: &mut Session, form: &str, morphs: Vec<MorphSpecView>) -> Result<TypeApplied, String> {
    let p: &mut Project = project(session)?;
    let mut specs: Vec<MorphSpec> = morphs.into_iter().map(Into::into).collect();
    let mut linked = 0;
    for spec in specs.iter_mut() {
        if spec.sense.is_some() {
            continue;
        }
        let Some(gloss) = spec.gloss.clone().filter(|g| !g.trim().is_empty()) else { continue };
        if let Some(sense) = sense_named(p, &spec.surface, &gloss)? {
            spec.sense = Some(sense);
            linked += 1;
        }
    }
    let r = p.apply_analysis_to_unanalysed(form, &specs)?;
    Ok(TypeApplied { applied: r.applied, texts: r.texts, linked })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unanalysed_word_types_are_listed_broken_and_analysed_once_for_all() {
        let dir = std::env::temp_dir().join(format!("wugs-gloss-types-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "prs_Ana").unwrap();
        let p = project(&mut s).unwrap();
        let text = p.proj.entities_of_kind("text").unwrap()[0].clone();
        let tier = p.proj.field_str(&p.proj.ordered_children("text", &text, "segment").unwrap()[0], "tier").unwrap().unwrap();
        // Two unbroken sentences, a word in both.
        p.add_segment(&text, &tier, 90, None, None, "kanu ra").unwrap();
        p.add_segment(&text, &tier, 91, None, None, "kanu kanu").unwrap();
        let before = word_types(&mut s, None).unwrap();
        assert_eq!((before.unbroken_sentences, before.unbroken_words), (2, 4));
        assert!(!before.rows.iter().any(|r| r.form == "kanu"), "unbroken words are not listed");
        assert_eq!(break_all(&mut s, None).unwrap(), 2);
        let after = word_types(&mut s, None).unwrap();
        assert_eq!(after.unbroken_sentences, 0);
        let kanu = after.rows.iter().find(|r| r.form == "kanu").unwrap();
        assert_eq!((kanu.unanalysed, kanu.uses), (3, 3));
        assert_eq!(after.rows[0].form, "kanu", "most unanalysed uses first");
        let spec = |surface: &str, t: &str, g: &str| MorphSpecView { surface: surface.into(), morph_type: t.into(), gloss: Some(g.into()), glosses_by_lang: Default::default(), sense: None };
        // A root the lexicon has, with that gloss, is linked to its sense.
        let p = project(&mut s).unwrap();
        let ((_, sing), _) = p.add_lexeme("nu", "sing").unwrap();
        let r = apply_type(&mut s, "kanu", vec![spec("ka", "prefix", "PST"), spec("nu", "root", "Sing")]).unwrap();
        assert_eq!((r.applied, r.linked), (3, 1));
        let p = project(&mut s).unwrap();
        let linked = p.proj.children_by_ref("sense", &sing, "morph").unwrap();
        assert_eq!(linked.len(), 3, "every use's root links to the sense");
        let last = word_types(&mut s, None).unwrap();
        assert!(!last.rows.iter().any(|r| r.form == "kanu"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
