//! The project's affixes and clitics as a grammar describes them: each one
//! with how often and on how many different roots it is used, the parts of
//! speech of those roots, and where it stands relative to the root, so the
//! words of the glossed texts give a position-class (slot) chart. Read-only,
//! over the same analyses the Concordance reads (`concordance::each_segment`).
//!
//! A word's root is its first morph typed `root` (else none, and the word is
//! skipped); every other morph is counted at its distance from the root,
//! −1 for the prefix right before it, +1 for the suffix or clitic right
//! after it. An affix is keyed by its folded surface (`lexicon_key`), its
//! type and its gloss, so `-ler` PL and `-ler` 3PL are two affixes; its slot
//! is the position it holds most often.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use crate::concordance::{bare, each_segment, morphs_of, Read, Scope};
use crate::session::{project, Session};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Affix {
    /// The commonest spelling, without markers.
    pub surface: String,
    /// `prefix`, `suffix`, `infix`, `clitic` or `reduplication`.
    pub kind: String,
    pub gloss: String,
    pub uses: usize,
    /// Different roots it is found on.
    pub roots: usize,
    /// Distance from the root → uses, nearest first on each side.
    pub positions: Vec<(i32, usize)>,
    /// The position it holds most often (ties: nearer the root).
    pub slot: i32,
    /// The roots' parts of speech (from their lexicon entries), commonest first.
    pub categories: Vec<(String, usize)>,
    /// A word it is in, its breakdown and the sentence it is from.
    pub example: Example,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Example {
    pub word: String,
    pub text_id: String,
    pub tier_id: String,
    pub segment_id: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Affixes {
    pub affixes: Vec<Affix>,
    /// Words with a root, which the chart is read from.
    pub words: usize,
    /// Words broken into morphs but with no morph typed as a root.
    pub without_root: usize,
}

#[derive(Default)]
struct Tally {
    spellings: HashMap<String, usize>,
    uses: usize,
    roots: HashSet<String>,
    positions: BTreeMap<i32, usize>,
    categories: HashMap<String, usize>,
    example: Option<Example>,
}

pub fn affixes(session: &mut Session) -> Result<Affixes, String> {
    let p = project(session)?;
    let mut out = Affixes::default();
    let mut tallies: HashMap<(String, String, String), Tally> = HashMap::new();
    let mut category_of: HashMap<String, Option<String>> = HashMap::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        for tok in &seg.tokens {
            let morphs = morphs_of(p, tok)?;
            if morphs.len() < 2 {
                continue;
            }
            let Some(root) = morphs.iter().position(|m| m.kind == "root") else {
                out.without_root += 1;
                continue;
            };
            out.words += 1;
            let root_key = wugs_model::lexicon_key(&p.proj.nfc(bare(&morphs[root].surface)));
            let category = match &morphs[root].sense {
                Some(sense) => match category_of.get(sense) {
                    Some(c) => c.clone(),
                    None => {
                        let c = match p.proj.field_str(sense, "lexeme")? {
                            Some(lex) => p.proj.field_str(&lex, "pos-of-speech")?.filter(|c| !c.trim().is_empty()),
                            None => None,
                        };
                        category_of.insert(sense.clone(), c.clone());
                        c
                    }
                },
                None => None,
            };
            let word = crate::concordance::joined(&morphs.iter().map(|m| (bare(&m.surface).to_string(), m.kind.clone())).collect::<Vec<_>>());
            for (i, m) in morphs.iter().enumerate() {
                if i == root || m.kind == "root" {
                    continue;
                }
                let surface = p.proj.nfc(bare(&m.surface));
                let key = (wugs_model::lexicon_key(&surface), m.kind.clone(), m.gloss.trim().to_string());
                let t = tallies.entry(key).or_default();
                *t.spellings.entry(surface).or_default() += 1;
                t.uses += 1;
                t.roots.insert(root_key.clone());
                *t.positions.entry(i as i32 - root as i32).or_default() += 1;
                if let Some(c) = &category {
                    *t.categories.entry(c.clone()).or_default() += 1;
                }
                if t.example.is_none() {
                    t.example = Some(Example { word: word.clone(), text_id: seg.text.clone(), tier_id: seg.tier.clone(), segment_id: seg.id.clone() });
                }
            }
        }
        Ok(())
    })?;
    for ((_, kind, gloss), t) in tallies {
        let surface = t.spellings.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|s| s.0.clone()).unwrap_or_default();
        let slot = t.positions.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.abs().cmp(&a.0.abs()))).map(|p| *p.0).unwrap_or(0);
        let mut positions: Vec<(i32, usize)> = t.positions.into_iter().collect();
        positions.sort_by_key(|p| (p.0.abs(), p.0));
        let mut categories: Vec<(String, usize)> = t.categories.into_iter().collect();
        categories.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out.affixes.push(Affix { surface, kind, gloss, uses: t.uses, roots: t.roots.len(), positions, slot, categories, example: t.example.unwrap_or(Example { word: String::new(), text_id: String::new(), tier_id: String::new(), segment_id: String::new() }) });
    }
    out.affixes.sort_by(|a, b| a.slot.cmp(&b.slot).then(b.uses.cmp(&a.uses)).then(a.surface.cmp(&b.surface)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gloss::MorphSpecView;

    #[test]
    fn the_glossed_words_give_a_slot_chart() {
        let dir = std::env::temp_dir().join(format!("wugs-affixes-test-{}", wugs_model::ulid()));
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        crate::sample::add_morphology(&mut session).unwrap();
        // One more word: ni-kani-ra=ma, so ni- is −1, -ra +1 and =ma +2 here.
        let text = crate::session::list_texts(&mut session).unwrap()[0].id.clone();
        let seg = crate::session::add_segment(&mut session, &text, None, Some(8_000.0), Some(9_000.0), "nikanirama").unwrap().id;
        let view = crate::gloss::gloss_tokenize(&mut session, &text, &seg, &["nikanirama".to_string()]).unwrap().view;
        let m = |surface: &str, kind: &str, gloss: &str| MorphSpecView { surface: surface.into(), morph_type: kind.into(), gloss: Some(gloss.into()), glosses_by_lang: Default::default(), sense: None };
        crate::gloss::gloss_apply(&mut session, &text, &seg, &view.words[0].id, vec![m("ni", "prefix", "1SG"), m("kani", "root", "count"), m("ra", "suffix", "PST"), m("ma", "clitic", "FOC")]).unwrap();
        let r = affixes(&mut session).unwrap();
        let find = |s: &str| r.affixes.iter().find(|a| a.surface == s).unwrap_or_else(|| panic!("{s} in {:?}", r.affixes));
        let ni = find("ni");
        assert_eq!((ni.kind.as_str(), ni.gloss.as_str(), ni.uses, ni.roots, ni.slot), ("prefix", "1SG", 2, 1, -1));
        let ma = find("ma");
        // =ma is +1 on «teka=ma» and +2 on «ni-kani-ra=ma»: a tie goes to the nearer slot.
        assert_eq!((ma.uses, ma.roots, ma.positions.clone(), ma.slot), (2, 2, vec![(1, 1), (2, 1)], 1));
        assert_eq!(find("ra").slot, 1);
        assert_eq!(r.affixes[0].surface, "ni", "prefixes come first, by slot");
        assert!(ni.example.word == "ni-kani" || ni.example.word == "ni-kani-ra=ma", "{:?}", ni.example);
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
