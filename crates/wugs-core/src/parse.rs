//! Guess an analysis for a word no one has analysed yet, from the morphs the
//! project already knows: every morph a person wrote in an analysis, and
//! every lexicon entry (an affix written with its marker, `-ler`, `=ma`,
//! `ni-`, else a root). A word is read as clitics and prefixes, one or two
//! roots, then suffixes and clitics, the way FLEx's parser reads a word from
//! its affix entries, but from what this project already glossed rather than
//! from a grammar someone wrote first.
//!
//! The guesses are suggestions only: nothing is written until a person
//! accepts one, which goes through `apply_analysis` like any analysis.
//! Ranked by fewest morphs, then by how often their morphs were used.

use std::collections::{BTreeMap, HashMap};

use wugs_model::Value;

use crate::gloss::{MorphSpec, Suggestion};
use crate::Project;

/// One morph the project knows, under its key (`lexicon_key`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownMorph {
    pub spec: MorphSpec,
    /// How many live analysed morphs carry exactly this morph.
    pub uses: usize,
}

/// The project's morphs by key, built once and asked many times.
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    by_key: HashMap<String, Vec<KnownMorph>>,
    /// Longest key in characters, so a parse never tries longer pieces.
    longest: usize,
}

/// Where in the word a morph may stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Slot {
    Before,
    Root,
    After,
}

/// At most this many guesses are returned for a word.
pub const GUESSES: usize = 3;
/// A word is cut into at most this many morphs.
const MAX_MORPHS: usize = 8;
/// Partial parses explored per word, so a long word with many one-letter
/// affixes stays quick.
const BUDGET: usize = 20_000;

fn slots(morph_type: &str) -> &'static [Slot] {
    match morph_type {
        "root" | "stem" | "" => &[Slot::Root],
        "prefix" | "proclitic" => &[Slot::Before],
        "suffix" | "enclitic" => &[Slot::After],
        // A clitic leans either way.
        "clitic" => &[Slot::Before, Slot::After],
        // Infixes and reduplication are not concatenative; never guessed.
        _ => &[],
    }
}

/// An entry's morph type from the marker its lemma is written with.
fn type_of_lemma(lemma: &str) -> &'static str {
    let t = lemma.trim();
    if t.starts_with('=') || t.ends_with('=') {
        "clitic"
    } else if t.starts_with('-') && !t.ends_with('-') {
        "suffix"
    } else if t.ends_with('-') && !t.starts_with('-') {
        "prefix"
    } else if t.starts_with(['~', '<']) || t.ends_with(['~', '>']) {
        "other"
    } else {
        "root"
    }
}

impl Inventory {
    fn add(&mut self, key: String, spec: MorphSpec, uses: usize) {
        if key.is_empty() || slots(&spec.morph_type).is_empty() {
            return;
        }
        self.longest = self.longest.max(key.chars().count());
        let list = self.by_key.entry(key).or_default();
        let same = |k: &KnownMorph| k.spec.morph_type == spec.morph_type && k.spec.gloss == spec.gloss && k.spec.sense == spec.sense;
        match list.iter_mut().find(|k| same(k)) {
            Some(k) => k.uses += uses,
            None => list.push(KnownMorph { spec, uses }),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    /// Guesses for a word already folded to its key (`lexicon_key` of NFC).
    pub fn guess(&self, key: &str) -> Vec<Suggestion> {
        let chars: Vec<char> = key.chars().collect();
        if chars.is_empty() || self.by_key.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<(Vec<&KnownMorph>, Vec<Slot>)> = Vec::new();
        let mut path: Vec<(&KnownMorph, Slot)> = Vec::new();
        let mut budget = BUDGET;
        self.walk(&chars, 0, &mut path, &mut found, &mut budget);
        // Fewest morphs first, then the most used morphs; the morphs' own order breaks ties.
        let weight = |m: &[&KnownMorph]| m.iter().map(|k| ((k.uses + 1) as f64).ln()).sum::<f64>();
        found.sort_by(|a, b| a.0.len().cmp(&b.0.len()).then(weight(&b.0).total_cmp(&weight(&a.0))));
        let mut out: Vec<Suggestion> = Vec::new();
        for (morphs, _) in found {
            let specs: Vec<MorphSpec> = morphs.iter().map(|k| k.spec.clone()).collect();
            if !out.iter().any(|s| s.morphs == specs) {
                out.push(Suggestion { morphs: specs, count: 0 });
            }
            if out.len() == GUESSES {
                break;
            }
        }
        out
    }

    fn walk<'a>(&'a self, chars: &[char], at: usize, path: &mut Vec<(&'a KnownMorph, Slot)>, found: &mut Vec<(Vec<&'a KnownMorph>, Vec<Slot>)>, budget: &mut usize) {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        if at == chars.len() {
            let roots = path.iter().filter(|(_, s)| *s == Slot::Root).count();
            if (1..=2).contains(&roots) {
                found.push((path.iter().map(|(k, _)| *k).collect(), path.iter().map(|(_, s)| *s).collect()));
            }
            return;
        }
        if path.len() == MAX_MORPHS {
            return;
        }
        let last = path.last().map(|(_, s)| *s);
        let roots = path.iter().filter(|(_, s)| *s == Slot::Root).count();
        let end = chars.len().min(at + self.longest);
        // Longer pieces first, so the fewest-morph parses come early.
        for to in (at + 1..=end).rev() {
            let piece: String = chars[at..to].iter().collect();
            let Some(list) = self.by_key.get(&piece) else { continue };
            for k in list {
                for &slot in slots(&k.spec.morph_type) {
                    // Slots never go back (prefixes, roots, suffixes), and two roots at most, side by side.
                    let fits = match (last, slot) {
                        (None, _) => true,
                        (Some(prev), next) if next < prev => false,
                        (Some(Slot::Root), Slot::Root) => roots < 2,
                        (Some(Slot::After), Slot::Root) | (Some(Slot::Root), Slot::Before) => false,
                        _ => true,
                    };
                    // A word must reach a root before its suffixes.
                    if !fits || (slot == Slot::After && roots == 0) {
                        continue;
                    }
                    path.push((k, slot));
                    self.walk(chars, to, path, found, budget);
                    path.pop();
                }
            }
        }
    }
}

impl Project {
    /// The morphs this project knows: every live morph of a live token with a
    /// surface (counted by use), and every live lexicon entry's lemma with its
    /// first sense (an affix by the marker it is written with), and its allomorphs.
    pub fn morph_inventory(&self) -> Result<Inventory, String> {
        let mut inv = Inventory::default();
        for morph in self.proj.entities_of_kind("morph")? {
            let mut token = None;
            for (field, value) in self.proj.fields_of(&morph)? {
                if field == "token" {
                    token = value.as_str().map(str::to_string);
                }
            }
            if token.as_deref().is_none_or(|t| self.proj.is_tombstoned(t).unwrap_or(true)) {
                continue;
            }
            let spec = self.morph_spec(&morph)?;
            if spec.surface.is_empty() {
                continue;
            }
            let key = wugs_model::lexicon_key(&self.proj.nfc(&spec.surface));
            let spec = MorphSpec { surface: wugs_model::MARKERS.iter().fold(spec.surface.clone(), |s, m| s.trim_matches(*m).to_string()), ..spec };
            inv.add(key, spec, 1);
        }
        for lexeme in self.proj.entities_of_kind("lexeme")? {
            let lemma = self.proj.field_str(&lexeme, "lemma")?.unwrap_or_default();
            let surface = lemma.trim().trim_matches(|c| wugs_model::MARKERS.contains(&c)).to_string();
            if surface.is_empty() {
                continue;
            }
            let Some(sense) = self.proj.ordered_children("lexeme", &lexeme, "sense")?.into_iter().next() else { continue };
            let mut glosses_by_lang = BTreeMap::new();
            let mut gloss = None;
            for (field, value) in self.proj.fields_of(&sense)? {
                let Some(v) = value.as_str().filter(|v| !v.is_empty()) else { continue };
                if field == "gloss" {
                    gloss = Some(v.to_string());
                } else if let Some(lang) = field.strip_prefix(wugs_model::GLOSS_LANG_PREFIX) {
                    glosses_by_lang.insert(lang.to_string(), v.to_string());
                }
            }
            let spec = MorphSpec { surface, morph_type: type_of_lemma(&lemma).to_string(), gloss, glosses_by_lang, sense: Some(sense) };
            // Its allomorphs (`-lar` beside `-ler`) are the same morph spelled another way;
            // one written without a marker takes the lemma's type.
            for (_, other) in self.allomorphs(&lexeme)? {
                let bare = other.trim().trim_matches(|c| wugs_model::MARKERS.contains(&c)).to_string();
                if bare.is_empty() {
                    continue;
                }
                let own = type_of_lemma(&other);
                let morph_type = if own == "root" { spec.morph_type.clone() } else { own.to_string() };
                inv.add(wugs_model::lexicon_key(&self.proj.nfc(&other)), MorphSpec { surface: bare, morph_type, ..spec.clone() }, 0);
            }
            inv.add(wugs_model::lexicon_key(&self.proj.nfc(&lemma)), spec, 0);
        }
        Ok(inv)
    }

    /// Guesses for `form` from `inventory` (`morph_inventory`), best first,
    /// at most `GUESSES`; each has `count` 0, since no token carries it yet.
    pub fn guess_analysis(&self, inventory: &Inventory, form: &str) -> Vec<Suggestion> {
        let nfc = match self.normalize("form", Value::str(form)) {
            Value::Str(s) => s,
            _ => form.to_string(),
        };
        inventory.guess(&wugs_model::lexicon_key(&nfc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(surface: &str, t: &str, gloss: &str) -> MorphSpec {
        MorphSpec { surface: surface.into(), morph_type: t.into(), gloss: Some(gloss.into()), ..MorphSpec::default() }
    }

    fn inv(list: &[(MorphSpec, usize)]) -> Inventory {
        let mut i = Inventory::default();
        for (spec, uses) in list {
            i.add(wugs_model::lexicon_key(&spec.surface), spec.clone(), *uses);
        }
        i
    }

    fn shown(s: &Suggestion) -> String {
        s.morphs.iter().map(|m| format!("{}:{}", m.surface, m.gloss.as_deref().unwrap_or("?"))).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn reads_prefixes_roots_suffixes_and_clitics() {
        let i = inv(&[
            (m("ni", "prefix", "1SG"), 5),
            (m("kani", "root", "count"), 3),
            (m("kesi", "root", "three"), 1),
            (m("ma", "clitic", "FOC"), 2),
            (m("ler", "suffix", "PL"), 4),
        ]);
        assert_eq!(i.guess("nikesi").iter().map(shown).collect::<Vec<_>>(), ["ni:1SG kesi:three"]);
        assert_eq!(i.guess("kesima").iter().map(shown).collect::<Vec<_>>(), ["kesi:three ma:FOC"]);
        assert_eq!(i.guess("nikaniler").iter().map(shown).collect::<Vec<_>>(), ["ni:1SG kani:count ler:PL"]);
        // A known root alone is a guess too (never analysed in this form).
        assert_eq!(i.guess("kesi").iter().map(shown).collect::<Vec<_>>(), ["kesi:three"]);
        // No root, or a suffix before the root: no guess.
        assert!(i.guess("nima").is_empty());
        assert!(i.guess("lerkesi").is_empty());
        assert!(i.guess("unknown").is_empty());
    }

    #[test]
    fn fewer_morphs_and_more_used_morphs_rank_first() {
        let i = inv(&[(m("a", "prefix", "A"), 1), (m("bc", "root", "x"), 1), (m("abc", "root", "whole"), 1), (m("ka", "suffix", "PL"), 9), (m("ka", "suffix", "DAT"), 2)]);
        let g = i.guess("abc");
        assert_eq!(shown(&g[0]), "abc:whole");
        assert_eq!(shown(&g[1]), "a:A bc:x");
        // Two glosses for one affix: the more used first, both offered.
        let g = i.guess("bcka").iter().map(shown).collect::<Vec<_>>();
        assert_eq!(g, ["bc:x ka:PL", "bc:x ka:DAT"]);
        assert!(i.guess("abcka").len() <= GUESSES);
    }

    #[test]
    fn affix_entries_read_their_type_from_their_marker() {
        assert_eq!(type_of_lemma("-ler"), "suffix");
        assert_eq!(type_of_lemma("ni-"), "prefix");
        assert_eq!(type_of_lemma("=ma"), "clitic");
        assert_eq!(type_of_lemma("kesi"), "root");
        assert_eq!(type_of_lemma("<um>"), "other");
    }

    #[test]
    fn a_long_word_stays_within_budget() {
        let mut list: Vec<(MorphSpec, usize)> = "abcdefghij".chars().map(|c| (m(&c.to_string(), "prefix", "X"), 1)).collect();
        list.push((m("a", "root", "r"), 1));
        list.push((m("a", "suffix", "S"), 1));
        let i = inv(&list);
        // 40 letters of one-letter morphs: many parses, a bounded search, never a hang.
        let _ = i.guess(&"a".repeat(40));
    }
}
