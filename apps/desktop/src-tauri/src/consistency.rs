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

/// A root glossed in the texts with no lexicon entry of its form: what a
/// dictionary is still missing, as FLEx's word list shows.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Missing {
    /// The spelling met most often.
    pub surface: String,
    /// `wugs_model::lexicon_key` of the surface.
    pub key: String,
    pub total: usize,
    /// Each gloss it carries, most used first; each would be a sense.
    pub variants: Vec<Variant>,
}

/// A live lexicon entry whose lemma or other form has this key.
fn known(p: &wugs_core::Project, key: &str) -> Result<bool, String> {
    if !p.proj.by_lookup_key("lexeme", key)?.is_empty() {
        return Ok(true);
    }
    for form in p.proj.by_lookup_key("form", key)? {
        if let Some(lex) = p.proj.field_str(&form, "lexeme")? {
            if p.proj.entity_kind(&lex)?.as_deref() == Some("lexeme") && !p.proj.is_tombstoned(&lex)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// The glossed roots with no sense link and no entry of their form, by
/// key: `(key, surfaces by use, gloss → variant, morphs as (text, morph, gloss))`.
type Unlisted = BTreeMap<String, (BTreeMap<String, usize>, BTreeMap<String, Variant>, Vec<(String, String, String)>)>;

fn unlisted(p: &wugs_core::Project) -> Result<Unlisted, String> {
    let mut found: Unlisted = BTreeMap::new();
    let mut known_keys: BTreeMap<String, bool> = BTreeMap::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        for tok in &seg.tokens {
            for m in p.proj.members(tok)? {
                if !matches!(p.proj.field_str(&m, "type")?.as_deref(), None | Some("root") | Some("")) {
                    continue;
                }
                let gloss = p.proj.field_str(&m, "gloss")?.unwrap_or_default().trim().to_string();
                if gloss.is_empty() {
                    continue;
                }
                if let Some(sense) = p.proj.field_str(&m, "sense")?.filter(|s| !s.is_empty()) {
                    if p.proj.entity_kind(&sense)?.as_deref() == Some("sense") && !p.proj.is_tombstoned(&sense)? {
                        continue;
                    }
                }
                let surface = p.proj.nfc(&p.proj.field_str(&m, "surface")?.unwrap_or_default());
                let key = wugs_model::lexicon_key(&surface);
                if key.is_empty() {
                    continue;
                }
                let is_known = match known_keys.get(&key) {
                    Some(k) => *k,
                    None => {
                        let k = known(p, &key)?;
                        known_keys.insert(key.clone(), k);
                        k
                    }
                };
                if is_known {
                    continue;
                }
                let e = found.entry(key).or_default();
                *e.0.entry(surface).or_default() += 1;
                let v = e.1.entry(gloss.clone()).or_insert_with(|| Variant { gloss: gloss.clone(), count: 0, text_id: seg.text.clone(), text_title: seg.title.clone(), tier_id: seg.tier.clone(), segment_id: seg.id.clone() });
                v.count += 1;
                e.2.push((seg.text.clone(), m, gloss));
            }
        }
        Ok(())
    })?;
    Ok(found)
}

/// Every glossed root with no entry in the lexicon, most used first.
pub fn missing(session: &mut Session) -> Result<Vec<Missing>, String> {
    let p = session::project(session)?;
    let mut out: Vec<Missing> = unlisted(p)?
        .into_iter()
        .map(|(key, (surfaces, glosses, _))| {
            let mut variants: Vec<Variant> = glosses.into_values().collect();
            variants.sort_by(|a, b| b.count.cmp(&a.count).then(a.gloss.cmp(&b.gloss)));
            let surface = surfaces.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(s, _)| s.clone()).unwrap_or_default();
            Missing { surface, key, total: variants.iter().map(|v| v.count).sum(), variants }
        })
        .collect();
    out.sort_by(|a, b| b.total.cmp(&a.total).then(a.key.cmp(&b.key)));
    out.truncate(LIMIT);
    Ok(out)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    pub entries: usize,
    pub senses: usize,
    /// Morphs now linked to the new senses.
    pub linked: usize,
}

/// Make an entry for each of these roots (`keys`; all of them when empty):
/// its most used spelling as the lemma, a sense per gloss it carries, and
/// every one of its morphs linked to the sense of its gloss.
pub fn add_missing(session: &mut Session, keys: &[String]) -> Result<Added, String> {
    let p = session::project(session)?;
    let mut added = Added { entries: 0, senses: 0, linked: 0 };
    for (key, (surfaces, glosses, morphs)) in unlisted(p)? {
        if !keys.is_empty() && !keys.contains(&key) {
            continue;
        }
        let lemma = surfaces.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(s, _)| s.clone()).unwrap_or_default();
        let mut by_use: Vec<(&String, usize)> = glosses.iter().map(|(g, v)| (g, v.count)).collect();
        by_use.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let mut sense_of: BTreeMap<String, String> = BTreeMap::new();
        let ((lex, first), _) = p.add_lexeme(&lemma, by_use[0].0)?;
        sense_of.insert(by_use[0].0.clone(), first);
        for (gloss, _) in &by_use[1..] {
            let (sen, _) = p.add_sense(&lex, gloss)?;
            sense_of.insert((*gloss).clone(), sen);
        }
        added.entries += 1;
        added.senses += sense_of.len();
        for (text, morph, gloss) in &morphs {
            p.link_sense(text, morph, &sense_of[gloss])?;
            added.linked += 1;
        }
    }
    Ok(added)
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
        // Roots glossed in the texts with no entry: to, re, se, and mi with its two meanings.
        let missing = missing(&mut s).unwrap();
        let keys: Vec<&str> = missing.iter().map(|m| m.key.as_str()).collect();
        assert_eq!(keys, ["mi", "re", "se", "to"], "most used first; the tutorial's own roots are linked: {keys:?}");
        assert_eq!(missing[0].variants.iter().map(|v| (v.gloss.as_str(), v.count)).collect::<Vec<_>>(), [("water", 2), ("drink", 1)]);
        let one = add_missing(&mut s, &["mi".to_string()]).unwrap();
        assert_eq!((one.entries, one.senses, one.linked), (1, 2, 3), "one entry, a sense per meaning, every use linked");
        let p = session::project(&mut s).unwrap();
        let lex = p.proj.by_lookup_key("lexeme", "mi").unwrap()[0].clone();
        let senses = p.proj.ordered_children("lexeme", &lex, "sense").unwrap();
        assert_eq!(senses.iter().map(|x| p.proj.field_str(x, "gloss").unwrap().unwrap()).collect::<Vec<_>>(), ["water", "drink"]);
        let rest = add_missing(&mut s, &[]).unwrap();
        assert_eq!((rest.entries, rest.linked), (3, 3));
        assert!(missing_is_empty(&mut s));
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }

    fn missing_is_empty(s: &mut Session) -> bool {
        missing(s).unwrap().is_empty()
    }
}
