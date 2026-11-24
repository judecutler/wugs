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

/// The language's own inventories' Glottocode, and the languages it is
/// compared with (each by its first inventory) with what they are in words.
fn group(me: &glottolog::Languoid) -> (&'static str, Vec<(glottolog::Languoid, &'static Inventory)>, String) {
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
    // Each other language once, by its first inventory.
    let firsts: Vec<(glottolog::Languoid, &Inventory)> = by_code
        .iter()
        .filter(|(code, _)| **code != own_code && **code != me.glottocode)
        .filter_map(|(code, list)| Some((glottolog::by_glottocode(code)?, list[0])))
        .collect();
    let pick = |keep: &dyn Fn(&glottolog::Languoid) -> bool| -> Vec<(glottolog::Languoid, &'static Inventory)> { firsts.iter().filter(|(l, _)| keep(l)).cloned().collect() };
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
    let own_code: &'static str = by_code.get_key_value(own_code).map(|(k, _)| *k).unwrap_or(me.glottocode);
    (own_code, others, compared_with)
}

pub fn compare(glottocode: &str) -> Result<Compared, String> {
    let me = glottolog::by_glottocode(glottocode).ok_or_else(|| format!("Glottolog has no languoid «{}».", glottocode.trim()))?;
    let (own_code, others, compared_with) = group(&me);
    let own = inventories()
        .iter()
        .filter(|i| i.glottocode == own_code)
        .map(|i| InventoryView { id: i.id.into(), source: i.source.into(), name: i.name.into(), phonemes: i.phonemes.iter().map(|p| p.to_string()).collect() })
        .collect();
    let others: Vec<&Inventory> = others.into_iter().map(|(_, i)| i).collect();
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

/// Which of the compared languages have one phoneme, on a map (the
/// Typology panel's map shape: code `1` has it, `0` lacks it); the language
/// itself by its first inventory, `?` when PHOIBLE has none for it.
pub fn map(glottocode: &str, phoneme: &str) -> Result<crate::typology::FeatureMap, String> {
    let me = glottolog::by_glottocode(glottocode).ok_or_else(|| format!("Glottolog has no languoid «{}».", glottocode.trim()))?;
    let (own_code, others, compared_with) = group(&me);
    let mut points = Vec::new();
    let mut unplaced = 0;
    for (l, inv) in &others {
        let value = if inv.phonemes.contains(&phoneme) { "1" } else { "0" };
        match (l.lat, l.lon) {
            (Some(lat), Some(lon)) => points.push(crate::typology::MapPoint { glottocode: l.glottocode.to_string(), name: l.name.to_string(), lat, lon, value: Some(value.into()), me: false }),
            _ => unplaced += 1,
        }
    }
    let own = inventories().iter().find(|i| i.glottocode == own_code);
    let at = [Some(me.clone()), glottolog::by_glottocode(own_code)].into_iter().flatten().find(|l| l.lat.is_some() && l.lon.is_some());
    if let Some(at) = at {
        let value = own.map(|i| if i.phonemes.contains(&phoneme) { "1" } else { "0" }).unwrap_or("?");
        points.push(crate::typology::MapPoint { glottocode: me.glottocode.to_string(), name: me.name.to_string(), lat: at.lat.unwrap_or(0.0), lon: at.lon.unwrap_or(0.0), value: Some(value.into()), me: true });
    }
    Ok(crate::typology::FeatureMap {
        feature: phoneme.to_string(),
        question: format!("Is /{phoneme}/ in the inventory?"),
        codes: vec![crate::typology::Code { value: "0".into(), label: format!("no /{phoneme}/") }, crate::typology::Code { value: "1".into(), label: format!("/{phoneme}/") }],
        compared_with,
        points,
        unplaced,
        land: crate::typology::LAND,
        edition: EDITION,
    })
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
        // On a map: as many compared languages with /m/ placed as the share counts, less the unplaced.
        let m = map("stan1288", "m").unwrap();
        let with: usize = m.points.iter().filter(|p| !p.me && p.value.as_deref() == Some("1")).count();
        let share = s.shares.iter().find(|x| x.phoneme == "m").unwrap().languages;
        assert!(with <= share && with + m.unplaced >= share);
        assert_eq!(m.points.iter().filter(|p| !p.me).count() + m.unplaced, s.relatives);
        assert!(m.points.last().is_some_and(|p| p.me && p.value.as_deref() == Some("1")), "Spanish has /m/");
        assert!(compare("nope1234").is_err());
    }
}
