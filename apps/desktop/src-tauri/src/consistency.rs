//! Gloss consistency (the Checks panel): the same morph glossed more than
//! one way across the project, as FLEx's "Analyses" and every grammar's
//! last proofread look for. `PL` beside `pl` or `PL.` is a slip; `PL`
//! beside `3SG` on an `-s` may be two morphs that sound alike, which the
//! linguist decides. Nothing here writes until the person picks a gloss to
//! use everywhere.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{self, Session};

/// One way a morph is glossed, with where it first occurs.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    pub gloss: String,
    pub count: usize,
    pub text_id: String,
    pub text_title: String,
    pub tier_id: String,
    pub segment_id: String,
}

/// A morph glossed more than one way.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inconsistent {
    /// The spelling met most often.
    pub surface: String,
    /// `root`, `prefix`, `suffix`, … (the same surface as another type is another morph).
    pub kind: String,
    /// What groups the morphs: `wugs_model::lexicon_key` of the surface.
    pub key: String,
    pub total: usize,
    /// Most used first.
    pub variants: Vec<Variant>,
    /// The glosses differ only in case, spaces or dots: almost surely a slip.
    pub slip: bool,
}

/// At most this many morphs are listed.
const LIMIT: usize = 500;

/// A gloss with case, spaces and separators taken away, to tell a slip
/// (`PL`, `pl`, `PL.`) from two meanings (`PL`, `3SG`).
pub fn slip_key(gloss: &str) -> String {
    gloss.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

struct Group {
    surfaces: BTreeMap<String, usize>,
    glosses: BTreeMap<String, Variant>,
    /// The senses each gloss's morphs link to (none: no link).
    senses: BTreeMap<String, BTreeSet<Option<String>>>,
}

/// Every morph on transcription tiers glossed more than one way, slips
/// first, then by how many uses the minority glosses have. Morphs of one
/// surface linked to different lexicon senses, one sense per gloss, are
/// homonyms the linguist already told apart, and are left out.
pub fn glosses(session: &mut Session) -> Result<Vec<Inconsistent>, String> {
    let p = session::project(session)?;
    let mut groups: BTreeMap<(String, String), Group> = BTreeMap::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        for tok in &seg.tokens {
            for m in p.proj.members(tok)? {
                let gloss = p.proj.field_str(&m, "gloss")?.unwrap_or_default().trim().to_string();
                if gloss.is_empty() {
                    continue;
                }
                let surface = p.proj.nfc(&p.proj.field_str(&m, "surface")?.unwrap_or_default());
                let key = wugs_model::lexicon_key(&surface);
                if key.is_empty() {
                    continue;
                }
                let kind = p.proj.field_str(&m, "type")?.unwrap_or_else(|| "root".into());
                let g = groups.entry((key, kind)).or_insert_with(|| Group { surfaces: BTreeMap::new(), glosses: BTreeMap::new(), senses: BTreeMap::new() });
                *g.surfaces.entry(surface).or_default() += 1;
                let v = g.glosses.entry(gloss.clone()).or_insert_with(|| Variant { gloss: gloss.clone(), count: 0, text_id: seg.text.clone(), text_title: seg.title.clone(), tier_id: seg.tier.clone(), segment_id: seg.id.clone() });
                v.count += 1;
                g.senses.entry(gloss).or_default().insert(p.proj.field_str(&m, "sense")?);
            }
        }
        Ok(())
    })?;
    let mut out = Vec::new();
    for ((key, kind), g) in groups {
        if g.glosses.len() < 2 {
            continue;
        }
        // Homonyms: every gloss's morphs link to one sense, a different one each.
        let one_each: Vec<&Option<String>> = g.senses.values().filter(|s| s.len() == 1).filter_map(|s| s.iter().next()).collect();
        if one_each.len() == g.senses.len() && one_each.iter().all(|s| s.is_some()) && one_each.iter().collect::<BTreeSet<_>>().len() == one_each.len() {
            continue;
        }
        let slip = g.glosses.keys().map(|k| slip_key(k)).collect::<BTreeSet<_>>().len() == 1;
        let mut variants: Vec<Variant> = g.glosses.into_values().collect();
        variants.sort_by(|a, b| b.count.cmp(&a.count).then(a.gloss.cmp(&b.gloss)));
        let total = variants.iter().map(|v| v.count).sum();
        let surface = g.surfaces.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(s, _)| s.clone()).unwrap_or_default();
        out.push(Inconsistent { surface, kind, key, total, variants, slip });
    }
    let minority = |i: &Inconsistent| i.total - i.variants[0].count;
    out.sort_by(|a, b| b.slip.cmp(&a.slip).then(minority(b).cmp(&minority(a))).then(a.key.cmp(&b.key)));
    out.truncate(LIMIT);
    Ok(out)
}

/// Gloss every morph of this surface and type that reads `from` as `to`
/// instead (a person's choice, so each write settles a conflict on it).
/// Returns how many changed.
pub fn unify(session: &mut Session, key: &str, kind: &str, from: &str, to: &str) -> Result<usize, String> {
    let to = to.trim();
    if to.is_empty() {
        return Err("Say which gloss to use.".into());
    }
    let p = session::project(session)?;
    let mut targets: Vec<(String, String)> = Vec::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        for tok in &seg.tokens {
            for m in p.proj.members(tok)? {
                let surface = p.proj.nfc(&p.proj.field_str(&m, "surface")?.unwrap_or_default());
                if wugs_model::lexicon_key(&surface) != key || p.proj.field_str(&m, "type")?.unwrap_or_else(|| "root".into()) != kind {
                    continue;
                }
                if p.proj.field_str(&m, "gloss")?.unwrap_or_default().trim() == from {
                    targets.push((seg.text.clone(), m));
                }
            }
        }
        Ok(())
    })?;
    for (text, m) in &targets {
        p.set_gloss(text, m, to, None)?;
    }
    Ok(targets.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gloss;

    #[test]
    fn slips_and_two_meanings_are_found_and_unified() {
        let dir = std::env::temp_dir().join(format!("wugs-consistency-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        let text = session::list_texts(&mut s).unwrap()[0].id.clone();
        let spec = |surface: &str, kind: &str, g: &str| gloss::MorphSpecView { surface: surface.into(), morph_type: kind.into(), gloss: Some(g.into()), glosses_by_lang: Default::default(), sense: None };
        // Three sentences: «-ka» as PL twice and «pl.» once (a slip); «mi» as 'water' and 'drink' (two meanings).
        for (i, (words, analyses)) in [
            (["toka", "mi"], [vec![spec("to", "root", "dog"), spec("ka", "suffix", "PL")], vec![spec("mi", "root", "water")]]),
            (["reka", "mi"], [vec![spec("re", "root", "cat"), spec("ka", "suffix", "PL")], vec![spec("mi", "root", "drink")]]),
            (["seka", "mi"], [vec![spec("se", "root", "bird"), spec("ka", "suffix", "pl.")], vec![spec("mi", "root", "water")]]),
        ]
        .into_iter()
        .enumerate()
        {
            let at = 30_000.0 + i as f64 * 2000.0;
            let seg = session::add_segment(&mut s, &text, None, Some(at), Some(at + 1000.0), &words.join(" ")).unwrap().id;
            let forms: Vec<String> = words.iter().map(|w| w.to_string()).collect();
            let view = gloss::gloss_tokenize(&mut s, &text, &seg, &forms).unwrap().view;
            for (w, morphs) in view.words.iter().zip(analyses) {
                gloss::gloss_apply(&mut s, &text, &seg, &w.id, morphs).unwrap();
            }
        }
        let found = glosses(&mut s).unwrap();
        let ka = found.iter().find(|i| i.key == "ka").expect("-ka found");
        assert!(ka.slip, "PL and pl. differ only in case and a dot");
        assert_eq!(ka.variants.iter().map(|v| (v.gloss.as_str(), v.count)).collect::<Vec<_>>(), [("PL", 2), ("pl.", 1)]);
        let mi = found.iter().find(|i| i.key == "mi").expect("mi found");
        assert!(!mi.slip);
        assert_eq!(found[0].key, "ka", "slips first");
        assert_eq!(unify(&mut s, "ka", "suffix", "pl.", "PL").unwrap(), 1);
        assert!(glosses(&mut s).unwrap().iter().all(|i| i.key != "ka"), "unified");
        assert_eq!(unify(&mut s, "ka", "root", "PL", "X").unwrap(), 0, "another type is another morph");
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
