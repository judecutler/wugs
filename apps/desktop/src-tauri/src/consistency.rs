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

/// One gloss as written, with how many morphs carry it, how many of those
/// are affixes or clitics (not roots), and where it first occurs.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossForm {
    pub gloss: String,
    pub count: usize,
    pub affixes: usize,
    pub text_id: String,
    pub text_title: String,
    pub tier_id: String,
    pub segment_id: String,
}

/// The most glossed forms listed by `gloss_forms`.
pub const MAX_FORMS: usize = 20_000;

/// Every gloss in the primary language as written (trimmed), the most used
/// first: what the Leipzig check reads (`src/checks/leipzig.ts`).
pub fn gloss_forms(session: &mut Session) -> Result<Vec<GlossForm>, String> {
    let p = session::project(session)?;
    let mut forms: BTreeMap<String, GlossForm> = BTreeMap::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        for tok in &seg.tokens {
            for m in p.proj.members(tok)? {
                let gloss = p.proj.field_str(&m, "gloss")?.unwrap_or_default().trim().to_string();
                if gloss.is_empty() {
                    continue;
                }
                let affix = p.proj.field_str(&m, "type")?.is_some_and(|t| t != "root" && !t.is_empty());
                let f = forms.entry(gloss.clone()).or_insert_with(|| GlossForm { gloss, count: 0, affixes: 0, text_id: seg.text.clone(), text_title: seg.title.clone(), tier_id: seg.tier.clone(), segment_id: seg.id.clone() });
                f.count += 1;
                f.affixes += usize::from(affix);
            }
        }
        Ok(())
    })?;
    let mut out: Vec<GlossForm> = forms.into_values().collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then(a.gloss.cmp(&b.gloss)));
    out.truncate(MAX_FORMS);
    Ok(out)
}

/// Gloss every morph whose gloss reads `from` (trimmed) as `to`, whatever the morph.
pub fn regloss(session: &mut Session, from: &str, to: &str) -> Result<usize, String> {
    let to = to.trim();
    if to.is_empty() {
        return Err("Say which gloss to use.".into());
    }
    let p = session::project(session)?;
    let mut targets: Vec<(String, String)> = Vec::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        for tok in &seg.tokens {
            for m in p.proj.members(tok)? {
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

/// One way a word is broken into morphs, with where it first occurs.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Breakdown {
    /// The morphs as written, joined at `-`.
    pub breakdown: String,
    /// Their glosses, joined the same way (`?` for none).
    pub glosses: String,
    pub count: usize,
    /// A word broken up this way: `use_breakdown` copies its analysis.
    pub token_id: String,
    pub text_id: String,
    pub text_title: String,
    pub tier_id: String,
    pub segment_id: String,
}

/// A word form analysed with more than one breakdown (`nikani` as
/// `ni-kani` here and `nikani` there).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Segmented {
    /// The spelling met most often.
    pub form: String,
    /// `wugs_model::fold_key` of the form, as the Concordance keys words.
    pub key: String,
    pub total: usize,
    /// Most used first.
    pub variants: Vec<Breakdown>,
}

/// Every analysed word form on transcription tracks broken into morphs in
/// more than one way (by the morphs' `lexicon_key`s, so `-ler` and `ler`
/// are one), the forms whose minority breakdowns have most uses first.
/// Words not analysed yet are the Words to gloss panel's, and left out.
pub fn segmentations(session: &mut Session) -> Result<Vec<Segmented>, String> {
    let p = session::project(session)?;
    struct Seen {
        spellings: BTreeMap<String, usize>,
        variants: BTreeMap<Vec<String>, Breakdown>,
    }
    let mut forms: BTreeMap<String, Seen> = BTreeMap::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        for tok in &seg.tokens {
            let morphs = p.proj.members(tok)?;
            if morphs.is_empty() {
                continue;
            }
            let form = p.proj.nfc(&p.proj.field_str(tok, "form")?.unwrap_or_default());
            let key = wugs_model::fold_key(&form);
            if key.is_empty() {
                continue;
            }
            let mut surfaces = Vec::new();
            let mut glosses = Vec::new();
            for m in &morphs {
                surfaces.push(p.proj.nfc(&p.proj.field_str(m, "surface")?.unwrap_or_default()));
                let g = p.proj.field_str(m, "gloss")?.unwrap_or_default().trim().to_string();
                glosses.push(if g.is_empty() { "?".to_string() } else { g });
            }
            let shape: Vec<String> = surfaces.iter().map(|s| wugs_model::lexicon_key(s)).collect();
            let seen = forms.entry(key).or_insert_with(|| Seen { spellings: BTreeMap::new(), variants: BTreeMap::new() });
            *seen.spellings.entry(form).or_default() += 1;
            let v = seen.variants.entry(shape).or_insert_with(|| Breakdown {
                breakdown: surfaces.join("-"),
                glosses: glosses.join("-"),
                count: 0,
                token_id: tok.clone(),
                text_id: seg.text.clone(),
                text_title: seg.title.clone(),
                tier_id: seg.tier.clone(),
                segment_id: seg.id.clone(),
            });
            v.count += 1;
        }
        Ok(())
    })?;
    let mut out: Vec<Segmented> = forms
        .into_iter()
        .filter(|(_, s)| s.variants.len() > 1)
        .map(|(key, s)| {
            let mut variants: Vec<Breakdown> = s.variants.into_values().collect();
            variants.sort_by(|a, b| b.count.cmp(&a.count).then(a.breakdown.cmp(&b.breakdown)));
            let form = s.spellings.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(f, _)| f.clone()).unwrap_or_default();
            Segmented { form, key, total: variants.iter().map(|v| v.count).sum(), variants }
        })
        .collect();
    let minority = |s: &Segmented| s.total - s.variants[0].count;
    out.sort_by(|a, b| minority(b).cmp(&minority(a)).then(a.key.cmp(&b.key)));
    out.truncate(LIMIT);
    Ok(out)
}

/// Give every analysed word of `token`'s form whose breakdown differs the
/// analysis `token` has (its morphs, types, glosses and sense links), one
/// person's choice each, so each write settles conflicts on it. Returns how
/// many words changed.
pub fn use_breakdown(session: &mut Session, token: &str) -> Result<usize, String> {
    let p = session::project(session)?;
    if p.proj.entity_kind(token)?.as_deref() != Some("token") || p.proj.is_tombstoned(token)? {
        return Err("That word is no longer in the project.".into());
    }
    let model = p.proj.members(token)?;
    if model.is_empty() {
        return Err("That word has no analysis to copy.".into());
    }
    let specs = model.iter().map(|m| p.morph_spec(m)).collect::<Result<Vec<_>, _>>()?;
    let shape_of = |morphs: &[String]| -> Result<Vec<String>, String> { morphs.iter().map(|m| Ok(wugs_model::lexicon_key(&p.proj.nfc(&p.proj.field_str(m, "surface")?.unwrap_or_default())))).collect() };
    let shape = shape_of(&model)?;
    let key = wugs_model::fold_key(&p.proj.nfc(&p.proj.field_str(token, "form")?.unwrap_or_default()));
    let mut targets: Vec<(String, String)> = Vec::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        for tok in &seg.tokens {
            if wugs_model::fold_key(&p.proj.nfc(&p.proj.field_str(tok, "form")?.unwrap_or_default())) != key {
                continue;
            }
            let morphs = p.proj.members(tok)?;
            if !morphs.is_empty() && shape_of(&morphs)? != shape {
                targets.push((seg.text.clone(), tok.clone()));
            }
        }
        Ok(())
    })?;
    for (text, tok) in &targets {
        p.apply_analysis(text, tok, &specs)?;
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
pub(crate) fn known(p: &wugs_core::Project, key: &str) -> Result<bool, String> {
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

/// Words glossed in the texts one way while the lexicon sense they are
/// linked to says another: the gloss in a text was retyped, or the entry
/// was edited after the text was glossed.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drift {
    pub sense_id: String,
    pub entry_id: String,
    pub headword: String,
    pub homograph: Option<u32>,
    /// The sense's gloss in the lexicon (may be empty).
    pub sense_gloss: String,
    /// The gloss the linked morphs carry in the texts.
    pub text_gloss: String,
    pub count: usize,
    /// Where it first occurs.
    pub first: Variant,
}

/// Does a gloss written in a text agree with its sense's gloss? Equal, or
/// one of the sense gloss's parts when it lists several (`dog; hound`).
pub fn fits(sense_gloss: &str, text_gloss: &str) -> bool {
    let s = sense_gloss.trim();
    let t = text_gloss.trim();
    s == t || s.split([';', ',']).any(|part| part.trim() == t)
}

/// Every linked morph's gloss that its sense does not have, grouped by
/// sense and gloss, most used first.
pub fn sense_drift(session: &mut Session) -> Result<Vec<Drift>, String> {
    let p = session::project(session)?;
    let mut groups: BTreeMap<(String, String), Drift> = BTreeMap::new();
    let mut senses: BTreeMap<String, Option<(String, String)>> = BTreeMap::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        for tok in &seg.tokens {
            for m in p.proj.members(tok)? {
                let Some(sense) = p.proj.field_str(&m, "sense")?.filter(|s| !s.is_empty()) else { continue };
                let gloss = p.proj.field_str(&m, "gloss")?.unwrap_or_default().trim().to_string();
                if gloss.is_empty() {
                    continue;
                }
                // The sense's gloss and entry, once per sense; none when either is gone.
                if !senses.contains_key(&sense) {
                    let live = p.proj.entity_kind(&sense)?.as_deref() == Some("sense") && !p.proj.is_tombstoned(&sense)?;
                    let entry = if live { p.proj.field_str(&sense, "lexeme")?.filter(|l| p.proj.is_tombstoned(l).map(|t| !t).unwrap_or(false)) } else { None };
                    let found = entry.map(|e| (e, p.proj.field_str(&sense, "gloss").ok().flatten().unwrap_or_default().trim().to_string()));
                    senses.insert(sense.clone(), found);
                }
                let Some(Some((entry, sense_gloss))) = senses.get(&sense) else { continue };
                if fits(sense_gloss, &gloss) {
                    continue;
                }
                let d = groups.entry((sense.clone(), gloss.clone())).or_insert_with(|| Drift {
                    sense_id: sense.clone(),
                    entry_id: entry.clone(),
                    headword: String::new(),
                    homograph: None,
                    sense_gloss: sense_gloss.clone(),
                    text_gloss: gloss.clone(),
                    count: 0,
                    first: Variant { gloss: gloss.clone(), count: 0, text_id: seg.text.clone(), text_title: seg.title.clone(), tier_id: seg.tier.clone(), segment_id: seg.id.clone() },
                });
                d.count += 1;
                d.first.count += 1;
            }
        }
        Ok(())
    })?;
    let mut out: Vec<Drift> = groups.into_values().collect();
    for d in &mut out {
        d.headword = crate::lexicon::str_field(p, &d.entry_id, "lemma")?;
        d.homograph = crate::lexicon::homograph(p, &d.entry_id)?;
    }
    out.sort_by(|a, b| b.count.cmp(&a.count).then(wugs_model::fold_key(&a.headword).cmp(&wugs_model::fold_key(&b.headword))).then(a.text_gloss.cmp(&b.text_gloss)));
    out.truncate(LIMIT);
    Ok(out)
}

/// Settle one drift: either every morph linked to `sense` and glossed
/// `text_gloss` takes the sense's gloss (`use_lexicon`), or the sense takes
/// `text_gloss`. Each write is a person's choice. Returns how many morphs
/// changed, or 1 for the lexicon edit.
pub fn settle_drift(session: &mut Session, sense: &str, text_gloss: &str, use_lexicon: bool) -> Result<usize, String> {
    let p = session::project(session)?;
    let text_gloss = text_gloss.trim();
    if !use_lexicon {
        if text_gloss.is_empty() {
            return Err("Say which gloss the lexicon should have.".into());
        }
        p.set_sense_gloss(sense, text_gloss, None)?;
        return Ok(1);
    }
    let to = p.proj.field_str(sense, "gloss")?.unwrap_or_default().trim().to_string();
    if to.is_empty() {
        return Err("The lexicon has no gloss for this sense yet: use the gloss from the texts instead.".into());
    }
    let mut targets: Vec<(String, String)> = Vec::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        for tok in &seg.tokens {
            for m in p.proj.members(tok)? {
                if p.proj.field_str(&m, "sense")?.as_deref() == Some(sense) && p.proj.field_str(&m, "gloss")?.unwrap_or_default().trim() == text_gloss {
                    targets.push((seg.text.clone(), m));
                }
            }
        }
        Ok(())
    })?;
    for (text, m) in &targets {
        p.set_gloss(text, m, &to, None)?;
    }
    Ok(targets.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gloss;

    #[test]
    fn gloss_forms_are_counted_and_reglossed_everywhere() {
        let dir = std::env::temp_dir().join(format!("wugs-forms-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        let text = session::list_texts(&mut s).unwrap()[0].id.clone();
        let spec = |surface: &str, kind: &str, g: &str| gloss::MorphSpecView { surface: surface.into(), morph_type: kind.into(), gloss: Some(g.into()), glosses_by_lang: Default::default(), sense: None };
        for (i, word) in ["kani", "mani"].iter().enumerate() {
            let at = 50_000.0 + i as f64 * 2000.0;
            let seg = session::add_segment(&mut s, &text, None, Some(at), Some(at + 1000.0), word).unwrap().id;
            let view = gloss::gloss_tokenize(&mut s, &text, &seg, &[word.to_string()]).unwrap().view;
            gloss::gloss_apply(&mut s, &text, &seg, &view.words[0].id, vec![spec(&word[..2], "root", "go out"), spec("ni", "suffix", "pl")]).unwrap();
        }
        let forms = gloss_forms(&mut s).unwrap();
        let pl = forms.iter().find(|f| f.gloss == "pl").unwrap();
        assert_eq!((pl.count, pl.affixes), (2, 2));
        let go = forms.iter().find(|f| f.gloss == "go out").unwrap();
        assert_eq!((go.count, go.affixes), (2, 0));
        assert_eq!(regloss(&mut s, "go out", "go.out").unwrap(), 2);
        assert!(gloss_forms(&mut s).unwrap().iter().any(|f| f.gloss == "go.out" && f.count == 2));
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_word_broken_up_two_ways_is_found_and_made_one() {
        let dir = std::env::temp_dir().join(format!("wugs-segmentations-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        let text = session::list_texts(&mut s).unwrap()[0].id.clone();
        let spec = |surface: &str, kind: &str, g: &str| gloss::MorphSpecView { surface: surface.into(), morph_type: kind.into(), gloss: Some(g.into()), glosses_by_lang: Default::default(), sense: None };
        // «Tokami» as to-ka-mi twice, as tokami once; «mi» alone stays one way.
        let mut tokens = Vec::new();
        for (i, (words, analyses)) in [
            (["tokami", "mi"], [vec![spec("to", "root", "dog"), spec("ka", "suffix", "PL"), spec("mi", "suffix", "ERG")], vec![spec("mi", "root", "water")]]),
            (["Tokami", "mi"], [vec![spec("to", "root", "dog"), spec("ka", "suffix", "PL"), spec("mi", "suffix", "ERG")], vec![spec("mi", "root", "water")]]),
            (["tokami", "mi"], [vec![spec("tokami", "root", "dogs")], vec![spec("mi", "root", "water")]]),
        ]
        .into_iter()
        .enumerate()
        {
            let at = 40_000.0 + i as f64 * 2000.0;
            let seg = session::add_segment(&mut s, &text, None, Some(at), Some(at + 1000.0), &words.join(" ")).unwrap().id;
            let forms: Vec<String> = words.iter().map(|w| w.to_string()).collect();
            let view = gloss::gloss_tokenize(&mut s, &text, &seg, &forms).unwrap().view;
            for (w, morphs) in view.words.iter().zip(analyses) {
                gloss::gloss_apply(&mut s, &text, &seg, &w.id, morphs).unwrap();
            }
            tokens.push(view.words[0].id.clone());
        }
        let found = segmentations(&mut s).unwrap();
        let tokami = found.iter().find(|f| f.key == "tokami").expect("tokami found");
        assert_eq!((tokami.form.as_str(), tokami.total), ("tokami", 3));
        assert_eq!(tokami.variants.iter().map(|v| (v.breakdown.as_str(), v.glosses.as_str(), v.count)).collect::<Vec<_>>(), [("to-ka-mi", "dog-PL-ERG", 2), ("tokami", "dogs", 1)]);
        assert!(!found.iter().any(|f| f.key == "mi"), "one breakdown is no finding");
        // The commoner breakdown for every word of the form.
        assert_eq!(use_breakdown(&mut s, &tokami.variants[0].token_id).unwrap(), 1);
        assert!(!segmentations(&mut s).unwrap().iter().any(|f| f.key == "tokami"));
        assert_eq!(use_breakdown(&mut s, &tokens[2]).unwrap(), 0, "nothing left to change");
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

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

    #[test]
    fn glosses_unlike_their_sense_are_found_and_settled() {
        assert!(fits("dog; hound", "hound") && fits("one", " one ") && !fits("one", "One"));
        let dir = std::env::temp_dir().join(format!("wugs-drift-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        assert!(sense_drift(&mut s).unwrap().is_empty(), "the tutorial agrees with its lexicon");
        // Retype one linked word's gloss in the text.
        let p = session::project(&mut s).unwrap();
        let mut first = None;
        each_segment(p, Read::Analyses, &Scope::all(), |seg| {
            for tok in &seg.tokens {
                for m in p.proj.members(tok)? {
                    if first.is_none() && p.proj.field_str(&m, "sense")?.is_some() {
                        first = Some((seg.text.clone(), m));
                    }
                }
            }
            Ok(())
        })
        .unwrap();
        let (text, morph) = first.expect("a linked morph");
        let sense = p.proj.field_str(&morph, "sense").unwrap().unwrap();
        let was = p.proj.field_str(&sense, "gloss").unwrap().unwrap();
        p.set_gloss(&text, &morph, "uno", None).unwrap();
        let d = sense_drift(&mut s).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].sense_gloss.as_str(), d[0].text_gloss.as_str(), d[0].count), (was.as_str(), "uno", 1));
        assert!(!d[0].headword.is_empty());
        // The lexicon's gloss back into the text.
        assert_eq!(settle_drift(&mut s, &sense, "uno", true).unwrap(), 1);
        assert!(sense_drift(&mut s).unwrap().is_empty());
        // Or the text's gloss into the lexicon.
        let p = session::project(&mut s).unwrap();
        p.set_gloss(&text, &morph, "uno", None).unwrap();
        assert_eq!(settle_drift(&mut s, &sense, "uno", false).unwrap(), 1);
        let p = session::project(&mut s).unwrap();
        assert_eq!(p.proj.field_str(&sense, "gloss").unwrap().as_deref(), Some("uno"));
        assert!(sense_drift(&mut s).unwrap().is_empty());
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }

    fn missing_is_empty(s: &mut Session) -> bool {
        missing(s).unwrap().is_empty()
    }
}
