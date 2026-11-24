//! PHOIBLE's phoneme inventories (datasets/phoible/, CC BY-SA 3.0), read for
//! the Sounds panel: the inventories PHOIBLE has of a language, and how often
//! each phoneme turns up in its relatives' inventories (its Glottolog family,
//! else its macroarea), so the sounds found in a project's transcriptions can
//! be set beside what is known. Comparing is done in the panel.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

use serde::Serialize;

use crate::glottolog;

/// The edition the table was built from, for the panel's credit line.
pub const EDITION: &str = "PHOIBLE 2.0 (CC BY-SA 3.0)";

/// A family with fewer languages in PHOIBLE than this is compared with its macroarea.
const MIN_RELATIVES: usize = 3;

struct Inventory {
    id: &'static str,
    glottocode: &'static str,
    source: &'static str,
    name: &'static str,
    phonemes: Vec<&'static str>,
}

fn inventories() -> &'static [Inventory] {
    static DATA: OnceLock<Vec<Inventory>> = OnceLock::new();
    DATA.get_or_init(|| {
        include_str!("../../../../datasets/phoible/inventories.tsv")
            .lines()
            .skip(1)
            .filter_map(|line| {
                let mut f = line.split('\t');
                Some(Inventory { id: f.next()?, glottocode: f.next()?, source: f.next()?, name: f.next()?, phonemes: f.next()?.split(' ').filter(|p| !p.is_empty()).collect() })
            })
            .collect()
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryView {
    pub id: String,
    /// The contributing database (UPSID, SAPHON, Erich Round's…), as PHOIBLE names it.
    pub source: String,
    pub name: String,
    pub phonemes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    pub phoneme: String,
    /// How many of the compared languages have it.
    pub languages: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Compared {
    pub name: String,
    /// The language's own inventories (a dialect PHOIBLE lacks read as its language).
    pub own: Vec<InventoryView>,
    /// What the shares count: a family's name or a macroarea, in words; empty when nothing.
    pub compared_with: String,
    /// Languages compared (each counted once, by its first inventory).
    pub relatives: usize,
    /// Every phoneme of the compared languages, the commonest first.
    pub shares: Vec<Share>,
    pub edition: &'static str,
}

pub fn compare(glottocode: &str) -> Result<Compared, String> {
    let me = glottolog::by_glottocode(glottocode).ok_or_else(|| format!("Glottolog has no languoid «{}».", glottocode.trim()))?;
    let all = inventories();
    let mut by_code: BTreeMap<&str, Vec<&Inventory>> = BTreeMap::new();
    for inv in all {
        by_code.entry(inv.glottocode).or_default().push(inv);
    }
    let own_code = if by_code.contains_key(me.glottocode) {
        me.glottocode
    } else if me.dialect && !me.language.is_empty() {
        glottolog::by_name(me.language, me.family).map(|l| l.glottocode).filter(|c| by_code.contains_key(c)).unwrap_or(me.glottocode)
    } else {
        me.glottocode
    };
    let own = by_code
        .get(own_code)
        .map(|list| list.iter().map(|i| InventoryView { id: i.id.into(), source: i.source.into(), name: i.name.into(), phonemes: i.phonemes.iter().map(|p| p.to_string()).collect() }).collect())
        .unwrap_or_default();
    // Each other language once, by its first inventory.
    let firsts: Vec<(glottolog::Languoid, &Inventory)> = by_code
        .iter()
        .filter(|(code, _)| **code != own_code && **code != me.glottocode)
        .filter_map(|(code, list)| Some((glottolog::by_glottocode(code)?, list[0])))
        .collect();
    let pick = |keep: &dyn Fn(&glottolog::Languoid) -> bool| -> Vec<&Inventory> { firsts.iter().filter(|(l, _)| keep(l)).map(|(_, i)| *i).collect() };
    let mut others = Vec::new();
    let mut compared_with = String::new();
    if !me.family.is_empty() && me.family != "isolate" {
        others = pick(&|l| l.family == me.family);
        compared_with = format!("{} languages", me.family);
    }
    if others.len() < MIN_RELATIVES && !me.macroarea.is_empty() {
        others = pick(&|l| l.macroarea == me.macroarea);
        compared_with = format!("languages of {}", me.macroarea);
    }
    if others.is_empty() {
        compared_with.clear();
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for inv in &others {
        for p in inv.phonemes.iter().collect::<HashSet<_>>() {
            *counts.entry(p).or_default() += 1;
        }
    }
    let mut shares: Vec<Share> = counts.into_iter().map(|(p, n)| Share { phoneme: p.to_string(), languages: n }).collect();
    shares.sort_by(|a, b| b.languages.cmp(&a.languages).then_with(|| a.phoneme.cmp(&b.phoneme)));
    Ok(Compared { name: me.name.to_string(), own, compared_with, relatives: others.len(), shares, edition: EDITION })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_language_with_inventories_and_relatives() {
        // Korean (kore1280) is PHOIBLE's inventory 1; Koreanic has Jeju too, so
        // a small family falls back to the macroarea.
        let c = compare("kore1280").unwrap();
        assert!(c.own.iter().any(|i| i.id == "1" && i.phonemes.iter().any(|p| p == "kʰ")));
        assert!(c.relatives >= MIN_RELATIVES);
        assert!(!c.compared_with.is_empty());
        assert!(c.shares.windows(2).all(|w| w[0].languages >= w[1].languages));
        assert!(c.shares[0].languages <= c.relatives);
        // A family with many languages is compared within the family.
        let s = compare("stan1288").unwrap();
        assert_eq!(s.compared_with, "Indo-European languages");
        assert!(s.shares.iter().any(|x| x.phoneme == "m" && x.languages * 10 > s.relatives * 9), "nearly every Indo-European inventory has m");
        assert!(compare("nope1234").is_err());
    }
}
