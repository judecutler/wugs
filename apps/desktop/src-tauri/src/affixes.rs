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
    /// What it attaches to, as written: the morphs before it in the word for
    /// an affix after the root, the morphs after it for one before the root,
    /// with how many uses; the commonest `MAX_STEMS`. What tells allomorphs
    /// of one gloss apart (the stem's last vowel, its last sound) is read
    /// from these.
    pub stems: Vec<(String, usize)>,
    /// Different word forms it is in, and how many of them occur only once
    /// in the glossed words: Baayen's productivity is `hapaxes / uses`.
    pub types: usize,
    pub hapaxes: usize,
}

/// At most this many stems are returned per affix.
pub const MAX_STEMS: usize = 100;

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
    /// Two affixes met in one word, `first` before `second` (indices into
    /// `affixes`), with how many words: the evidence for their order and for
    /// whether two affixes can share a slot. Most frequent first, at most `MAX_ORDERS`.
    pub orders: Vec<Order>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub first: usize,
    pub second: usize,
    pub count: usize,
}

/// At most this many ordered pairs are returned.
pub const MAX_ORDERS: usize = 500;

type Key = (String, String, String);

#[derive(Default)]
struct Tally {
    spellings: HashMap<String, usize>,
    uses: usize,
    roots: HashSet<String>,
    positions: BTreeMap<i32, usize>,
    categories: HashMap<String, usize>,
    example: Option<Example>,
    stems: HashMap<String, usize>,
    /// The word forms it is in (folded), with uses.
    words: HashMap<String, usize>,
}

pub fn affixes(session: &mut Session) -> Result<Affixes, String> {
    let p = project(session)?;
    let mut out = Affixes::default();
    let mut tallies: HashMap<Key, Tally> = HashMap::new();
    let mut pairs: HashMap<(Key, Key), usize> = HashMap::new();
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
            let mut in_word: Vec<Key> = Vec::new();
            let word = crate::concordance::joined(&morphs.iter().map(|m| (bare(&m.surface).to_string(), m.kind.clone())).collect::<Vec<_>>());
            let plain: Vec<String> = morphs.iter().map(|m| p.proj.nfc(bare(&m.surface))).collect();
            let word_key = wugs_model::fold_key(&plain.concat());
            for (i, m) in morphs.iter().enumerate() {
                if i == root || m.kind == "root" {
                    continue;
                }
                let surface = p.proj.nfc(bare(&m.surface));
                let key = (wugs_model::lexicon_key(&surface), m.kind.clone(), m.gloss.trim().to_string());
                in_word.push(key.clone());
                let t = tallies.entry(key).or_default();
                *t.spellings.entry(surface).or_default() += 1;
                t.uses += 1;
                t.roots.insert(root_key.clone());
                *t.positions.entry(i as i32 - root as i32).or_default() += 1;
                if let Some(c) = &category {
                    *t.categories.entry(c.clone()).or_default() += 1;
                }
                let stem = if i > root { plain[..i].concat() } else { plain[i + 1..].concat() };
                *t.stems.entry(stem).or_default() += 1;
                *t.words.entry(word_key.clone()).or_default() += 1;
                if t.example.is_none() {
                    t.example = Some(Example { word: word.clone(), text_id: seg.text.clone(), tier_id: seg.tier.clone(), segment_id: seg.id.clone() });
                }
            }
            // Every two affixes of the word in the order they stand, once per word.
            let mut seen = HashSet::new();
            for (i, a) in in_word.iter().enumerate() {
                for b in &in_word[i + 1..] {
                    if a != b && seen.insert((a.clone(), b.clone())) {
                        *pairs.entry((a.clone(), b.clone())).or_default() += 1;
                    }
                }
            }
        }
        Ok(())
    })?;
    let mut keyed: Vec<(Key, Affix)> = Vec::new();
    for (key, t) in tallies {
        let (_, kind, gloss) = key.clone();
        let surface = t.spellings.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|s| s.0.clone()).unwrap_or_default();
        let slot = t.positions.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.abs().cmp(&a.0.abs()))).map(|p| *p.0).unwrap_or(0);
        let mut positions: Vec<(i32, usize)> = t.positions.into_iter().collect();
        positions.sort_by_key(|p| (p.0.abs(), p.0));
        let mut categories: Vec<(String, usize)> = t.categories.into_iter().collect();
        categories.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut stems: Vec<(String, usize)> = t.stems.into_iter().collect();
        stems.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        stems.truncate(MAX_STEMS);
        let (types, hapaxes) = (t.words.len(), t.words.values().filter(|n| **n == 1).count());
        keyed.push((key, Affix { surface, kind, gloss, uses: t.uses, roots: t.roots.len(), positions, slot, categories, example: t.example.unwrap_or(Example { word: String::new(), text_id: String::new(), tier_id: String::new(), segment_id: String::new() }), stems, types, hapaxes }));
    }
    keyed.sort_by(|(ka, a), (kb, b)| a.slot.cmp(&b.slot).then(b.uses.cmp(&a.uses)).then(a.surface.cmp(&b.surface)).then(ka.cmp(kb)));
    let index: HashMap<&Key, usize> = keyed.iter().enumerate().map(|(i, (k, _))| (k, i)).collect();
    let mut orders: Vec<Order> = pairs.iter().map(|((a, b), n)| Order { first: index[a], second: index[b], count: *n }).collect();
    orders.sort_by(|x, y| y.count.cmp(&x.count).then(x.first.cmp(&y.first)).then(x.second.cmp(&y.second)));
    orders.truncate(MAX_ORDERS);
    out.orders = orders;
    out.affixes = keyed.into_iter().map(|(_, a)| a).collect();
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
        // Order: in ni-kani-ra=ma, ni- before -ra before =ma, each pair once.
        let at = |s: &str| r.affixes.iter().position(|a| a.surface == s).unwrap();
        let count = |a: &str, b: &str| r.orders.iter().find(|o| o.first == at(a) && o.second == at(b)).map(|o| o.count);
        assert_eq!(count("ni", "ra"), Some(1));
        assert_eq!(count("ra", "ma"), Some(1));
        assert_eq!(count("ni", "ma"), Some(1));
        assert_eq!(count("ma", "ra"), None, "never the other way");
        // Stems: what each affix attaches to, on the root's side.
        assert!(find("ra").stems.contains(&("nikani".to_string(), 1)), "{:?}", find("ra").stems);
        assert!(find("ma").stems.contains(&("nikanira".to_string(), 1)), "{:?}", find("ma").stems);
        assert!(ni.stems.iter().any(|(s, _)| s == "kanirama"), "{:?}", ni.stems);
        // Productivity: -ra is in one word form, met once.
        assert_eq!((find("ra").types, find("ra").hapaxes), (1, 1));
        assert!(ni.types >= 1 && ni.hapaxes <= ni.types, "{ni:?}");
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
