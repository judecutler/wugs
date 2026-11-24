use std::collections::{BTreeMap, HashMap, HashSet};

use wugs_core::Project;
use wugs_model::id::now_millis;

use crate::mapping::*;
use crate::xml::XmlNode;

struct Seg {
    id: String,
    tier: String,
    pos: String,
    start: Option<f64>,
    end: Option<f64>,
    parent: Option<String>,
    content: String,
    origin: Option<String>,
}

struct Tier {
    id: String,
    name: String,
    kind: String,
    parent: Option<String>,
    origin: Option<String>,
    lt_ref: Option<String>,
    constraints: Option<String>,
    participant: Option<String>,
    order: f64,
}

/// Render a text as an `.eaf` document. Reads the projection only.
pub fn export_eaf(project: &Project, text: &str) -> Result<String, String> {
    export_eaf_with_media(project, text, None)
}

/// A recording for ELAN to open with the file: its absolute `file:` URL and
/// the path relative to the `.eaf`, which ELAN tries when the first is gone.
pub struct Media<'a> {
    pub url: &'a str,
    pub relative: &'a str,
    pub mime: &'a str,
}

/// `export_eaf`, with a `MEDIA_DESCRIPTOR` naming the recording when given.
pub fn export_eaf_with_media(project: &Project, text: &str, media: Option<Media>) -> Result<String, String> {
    let p = &project.proj;
    if p.entity_kind(text)?.as_deref() != Some("text") {
        return Err(format!("{text} is not a text"));
    }

    let mut segs = Vec::new();
    for id in p.children_by_ref("text", text, "segment")? {
        segs.push(Seg {
            tier: p.field_str(&id, "tier")?.unwrap_or_default(),
            pos: p.field_str(&id, "pos")?.unwrap_or_default(),
            start: p.field_num(&id, "start")?,
            end: p.field_num(&id, "end")?,
            parent: p.field_str(&id, "parent")?,
            content: p.field_str(&id, "content")?.unwrap_or_default(),
            origin: p.field_str(&id, F_ORIGIN)?,
            id,
        });
    }
    segs.sort_by(|a, b| (&a.pos, &a.id).cmp(&(&b.pos, &b.id)));

    // Referenced tiers plus their ancestors; a tier id with no TierDef entity
    // still exports (born-digital data may reference one that was never made).
    let mut tiers: Vec<Tier> = Vec::new();
    let mut queue: Vec<String> = segs.iter().map(|s| s.tier.clone()).collect();
    let mut seen = HashSet::new();
    while let Some(id) = queue.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let known = p.entity_kind(&id)?.as_deref() == Some("tier");
        let parent = if known { p.field_str(&id, "parent")? } else { None };
        if let Some(par) = &parent {
            queue.push(par.clone());
        }
        tiers.push(Tier {
            name: p.field_str(&id, "name")?.unwrap_or_else(|| id.clone()),
            kind: p.field_str(&id, "kind")?.unwrap_or_else(|| KIND_ALIGNED.into()),
            origin: p.field_str(&id, F_ORIGIN)?,
            lt_ref: p.field_str(&id, F_LING_TYPE)?,
            constraints: p.field_str(&id, F_CONSTRAINTS)?,
            participant: p.field_str(&id, F_PARTICIPANT)?,
            order: p.field_num(&id, F_TIER_ORDER)?.unwrap_or(f64::MAX),
            parent,
            id,
        });
    }
    tiers.sort_by(|a, b| a.order.total_cmp(&b.order).then_with(|| a.name.cmp(&b.name)));

    let mut used_tier_ids = HashSet::new();
    let mut tier_eaf: HashMap<&str, String> = HashMap::new();
    for t in &tiers {
        let base = t.origin.clone().unwrap_or_else(|| t.name.clone());
        tier_eaf.insert(&t.id, unique(&base, &mut used_tier_ids));
    }

    let mut used_ann_ids: HashSet<String> = segs.iter().filter_map(|s| s.origin.clone()).collect();
    let mut ann_eaf: HashMap<&str, String> = HashMap::new();
    let mut counter = 0;
    for s in &segs {
        let id = s.origin.clone().unwrap_or_else(|| loop {
            counter += 1;
            let candidate = format!("a{counter}");
            if used_ann_ids.insert(candidate.clone()) {
                break candidate;
            }
        });
        ann_eaf.insert(&s.id, id);
    }

    // A segment is a REF_ANNOTATION only when its parent segment is in this text.
    let is_ref = |s: &Seg| s.parent.as_ref().is_some_and(|par| ann_eaf.contains_key(par.as_str()));

    // Valued slots first, in ascending time, so the document is monotone;
    // unvalued slots follow, one per missing boundary. ELAN interpolates
    // those by list position, so a file with many unvalued boundaries may
    // interpolate differently than the original.
    let mut times: Vec<f64> = segs.iter().filter(|s| !is_ref(s)).flat_map(|s| [s.start, s.end]).flatten().collect();
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| a.to_bits() == b.to_bits());
    let mut slot_nodes = Vec::new();
    let mut slot_for: HashMap<u64, String> = HashMap::new();
    for t in &times {
        let id = format!("ts{}", slot_nodes.len() + 1);
        slot_nodes.push(
            XmlNode::new("TIME_SLOT")
                .with_attr("TIME_SLOT_ID", &id)
                .with_attr("TIME_VALUE", &t.to_string()),
        );
        slot_for.insert(t.to_bits(), id);
    }
    let mut slot_ref = |t: Option<f64>| match t {
        Some(t) => slot_for[&t.to_bits()].clone(),
        None => {
            let id = format!("ts{}", slot_nodes.len() + 1);
            slot_nodes.push(XmlNode::new("TIME_SLOT").with_attr("TIME_SLOT_ID", &id));
            id
        }
    };

    let mut tier_nodes = Vec::new();
    let mut types: BTreeMap<String, (bool, Option<String>)> = BTreeMap::new();
    for t in &tiers {
        let constraints = t
            .constraints
            .clone()
            .or_else(|| default_constraints(&t.kind).map(str::to_string));
        let lt = t.lt_ref.clone().unwrap_or_else(|| default_type_id(&t.kind).to_string());
        types
            .entry(lt.clone())
            .or_insert((t.kind == KIND_ALIGNED, constraints));

        let mut node = XmlNode::new("TIER")
            .with_attr("LINGUISTIC_TYPE_REF", &lt)
            .with_attr("TIER_ID", &tier_eaf[t.id.as_str()]);
        if let Some(par) = &t.parent {
            node = node.with_attr("PARENT_REF", &tier_eaf[par.as_str()]);
        }
        if let Some(pt) = &t.participant {
            node = node.with_attr("PARTICIPANT", pt);
        }

        let mut last_in_group: HashMap<&str, &str> = HashMap::new();
        for s in segs.iter().filter(|s| s.tier == t.id) {
            let id = &ann_eaf[s.id.as_str()];
            let inner = if is_ref(s) {
                let par = s.parent.as_deref().unwrap();
                let mut n = XmlNode::new("REF_ANNOTATION")
                    .with_attr("ANNOTATION_ID", id)
                    .with_attr("ANNOTATION_REF", &ann_eaf[par]);
                if t.kind == KIND_SUBDIVISION {
                    if let Some(prev) = last_in_group.insert(par, id) {
                        n = n.with_attr("PREVIOUS_ANNOTATION", prev);
                    }
                }
                n
            } else {
                XmlNode::new("ALIGNABLE_ANNOTATION")
                    .with_attr("ANNOTATION_ID", id)
                    .with_attr("TIME_SLOT_REF1", &slot_ref(s.start))
                    .with_attr("TIME_SLOT_REF2", &slot_ref(s.end))
            };
            node = node.with_child(
                XmlNode::new("ANNOTATION")
                    .with_child(inner.with_child(XmlNode::new("ANNOTATION_VALUE").with_text(&s.content))),
            );
        }
        tier_nodes.push(node);
    }

    // The glossing as ELAN's usual interlinear tiers under each tier whose
    // segments hold words: words (a subdivision of the segment), morphs (a
    // subdivision of the word) and glosses (an association of the morph).
    let mut analysis_types = false;
    for t in &tiers {
        let mut words = XmlNode::new("TIER").with_attr("LINGUISTIC_TYPE_REF", TYPE_WORDS);
        let mut morphs = XmlNode::new("TIER").with_attr("LINGUISTIC_TYPE_REF", TYPE_MORPHS);
        let mut glosses = XmlNode::new("TIER").with_attr("LINGUISTIC_TYPE_REF", TYPE_GLOSSES);
        let mut any_morph = false;
        let mut any_word = false;
        // Tiers an ELAN file kept on its words or morphs (a part of speech), by name.
        let mut word_notes: BTreeMap<String, XmlNode> = BTreeMap::new();
        let mut morph_notes: BTreeMap<String, XmlNode> = BTreeMap::new();
        let mut fresh = |prefix: &str| loop {
            counter += 1;
            let candidate = format!("{prefix}{counter}");
            if used_ann_ids.insert(candidate.clone()) {
                break candidate;
            }
        };
        for s in segs.iter().filter(|s| s.tier == t.id) {
            let tokens: Vec<String> = p.members(&s.id)?.into_iter().filter(|tok| !p.is_punct_token(tok) && !p.is_tombstoned(tok).unwrap_or(true)).collect();
            let mut prev_word: Option<String> = None;
            for tok in tokens {
                let wid = fresh("w");
                let mut wn = XmlNode::new("REF_ANNOTATION").with_attr("ANNOTATION_ID", &wid).with_attr("ANNOTATION_REF", &ann_eaf[s.id.as_str()]);
                if let Some(prev) = prev_word.replace(wid.clone()) {
                    wn = wn.with_attr("PREVIOUS_ANNOTATION", &prev);
                }
                let form = p.field_str(&tok, "form")?.unwrap_or_default();
                words = words.with_child(XmlNode::new("ANNOTATION").with_child(wn.with_child(XmlNode::new("ANNOTATION_VALUE").with_text(&form))));
                any_word = true;
                kept_notes(p, &tok, &wid, TYPE_WORD_NOTES, &mut word_notes, &mut fresh)?;
                let ms: Vec<String> = p.members(&tok)?.into_iter().filter(|m| !p.is_tombstoned(m).unwrap_or(true)).collect();
                let root_at = ms.iter().position(|m| p.field_str(m, "type").ok().flatten().as_deref() == Some("root"));
                let mut prev_morph: Option<String> = None;
                for (i, m) in ms.iter().enumerate() {
                    let mid = fresh("m");
                    let mut mn = XmlNode::new("REF_ANNOTATION").with_attr("ANNOTATION_ID", &mid).with_attr("ANNOTATION_REF", &wid);
                    if let Some(prev) = prev_morph.replace(mid.clone()) {
                        mn = mn.with_attr("PREVIOUS_ANNOTATION", &prev);
                    }
                    let surface = p.field_str(m, "surface")?.unwrap_or_default();
                    let kind = p.field_str(m, "type")?.unwrap_or_default();
                    let shown = marked(&surface, &kind, root_at.is_some_and(|r| i < r));
                    morphs = morphs.with_child(XmlNode::new("ANNOTATION").with_child(mn.with_child(XmlNode::new("ANNOTATION_VALUE").with_text(&shown))));
                    let gloss = p.field_str(m, "gloss")?.unwrap_or_default();
                    let gid = fresh("g");
                    let gn = XmlNode::new("REF_ANNOTATION").with_attr("ANNOTATION_ID", &gid).with_attr("ANNOTATION_REF", &mid);
                    glosses = glosses.with_child(XmlNode::new("ANNOTATION").with_child(gn.with_child(XmlNode::new("ANNOTATION_VALUE").with_text(&gloss))));
                    any_morph = true;
                    kept_notes(p, m, &mid, TYPE_MORPH_NOTES, &mut morph_notes, &mut fresh)?;
                }
            }
        }
        if !any_word {
            continue;
        }
        analysis_types = true;
        let base = &tier_eaf[t.id.as_str()];
        let words_id = unique(&format!("{base} words"), &mut used_tier_ids);
        words = words.with_attr("TIER_ID", &words_id).with_attr("PARENT_REF", base);
        if let Some(pt) = &t.participant {
            words = words.with_attr("PARTICIPANT", pt);
        }
        tier_nodes.push(words);
        for (name, node) in word_notes {
            let id = unique(&name, &mut used_tier_ids);
            tier_nodes.push(node.with_attr("TIER_ID", &id).with_attr("PARENT_REF", &words_id));
            types.insert(TYPE_WORD_NOTES.into(), (false, Some("Symbolic_Association".into())));
        }
        if any_morph {
            let morphs_id = unique(&format!("{base} morphs"), &mut used_tier_ids);
            tier_nodes.push(morphs.with_attr("TIER_ID", &morphs_id).with_attr("PARENT_REF", &words_id));
            let glosses_id = unique(&format!("{base} glosses"), &mut used_tier_ids);
            tier_nodes.push(glosses.with_attr("TIER_ID", &glosses_id).with_attr("PARENT_REF", &morphs_id));
            for (name, node) in morph_notes {
                let id = unique(&name, &mut used_tier_ids);
                tier_nodes.push(node.with_attr("TIER_ID", &id).with_attr("PARENT_REF", &morphs_id));
                types.insert(TYPE_MORPH_NOTES.into(), (false, Some("Symbolic_Association".into())));
            }
        }
    }
    if analysis_types {
        types.insert(TYPE_WORDS.into(), (false, Some("Symbolic_Subdivision".into())));
        types.insert(TYPE_MORPHS.into(), (false, Some("Symbolic_Subdivision".into())));
        types.insert(TYPE_GLOSSES.into(), (false, Some("Symbolic_Association".into())));
    }

    let mut header = XmlNode::new("HEADER").with_attr("MEDIA_FILE", "").with_attr("TIME_UNITS", "milliseconds");
    // The schema puts media descriptors before the properties.
    if let Some(m) = &media {
        header = header.with_child(XmlNode::new("MEDIA_DESCRIPTOR").with_attr("MEDIA_URL", m.url).with_attr("MIME_TYPE", m.mime).with_attr("RELATIVE_MEDIA_URL", m.relative));
    }
    header = header.with_child(XmlNode::new("PROPERTY").with_attr("NAME", PROP_TEXT).with_text(text));
    if let Some(title) = p.field_str(text, "title")? {
        header = header.with_child(XmlNode::new("PROPERTY").with_attr("NAME", PROP_TITLE).with_text(&title));
    }

    let mut doc = XmlNode::new("ANNOTATION_DOCUMENT")
        .with_attr("AUTHOR", "")
        .with_attr("DATE", &iso_date(now_millis()))
        .with_attr("FORMAT", "3.0")
        .with_attr("VERSION", "3.0")
        .with_attr("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance")
        .with_attr("xsi:noNamespaceSchemaLocation", "http://www.mpi.nl/tools/elan/EAFv3.0.xsd")
        .with_child(header)
        .with_child(XmlNode {
            name: "TIME_ORDER".into(),
            children: slot_nodes,
            ..XmlNode::default()
        });
    doc.children.extend(tier_nodes);
    for (id, (alignable, constraints)) in &types {
        let mut lt = XmlNode::new("LINGUISTIC_TYPE")
            .with_attr("GRAPHIC_REFERENCES", "false")
            .with_attr("LINGUISTIC_TYPE_ID", id)
            .with_attr("TIME_ALIGNABLE", if *alignable { "true" } else { "false" });
        if let Some(c) = constraints {
            lt = lt.with_attr("CONSTRAINTS", c);
        }
        doc.children.push(lt);
    }
    for (desc, stereotype) in ELAN_CONSTRAINTS {
        doc.children.push(
            XmlNode::new("CONSTRAINT")
                .with_attr("DESCRIPTION", desc)
                .with_attr("STEREOTYPE", stereotype),
        );
    }
    Ok(doc.to_xml())
}

/// A word's or morph's `x.elan.<tier>` fields, each an annotation on its own
/// tier under the word or morph (`ann` its annotation id here).
fn kept_notes(p: &wugs_projection::Projection, of: &str, ann: &str, kind: &str, tiers: &mut BTreeMap<String, XmlNode>, fresh: &mut impl FnMut(&str) -> String) -> Result<(), String> {
    for (field, value) in p.fields_of(of)? {
        let Some(name) = field.strip_prefix(F_KEPT_TIER) else { continue };
        let Some(text) = value.as_str().filter(|t| !t.is_empty()) else { continue };
        let id = fresh("x");
        let node = XmlNode::new("REF_ANNOTATION").with_attr("ANNOTATION_ID", &id).with_attr("ANNOTATION_REF", ann);
        let tier = tiers.entry(name.to_string()).or_insert_with(|| XmlNode::new("TIER").with_attr("LINGUISTIC_TYPE_REF", kind));
        tier.children.push(XmlNode::new("ANNOTATION").with_child(node.with_child(XmlNode::new("ANNOTATION_VALUE").with_text(text))));
    }
    Ok(())
}

/// A morph as an interlinear line shows it: its Leipzig marker on the side
/// that joins the root (`ni-`, `-ler`, `=ma`, `<um>`); a surface that already
/// carries one, as FLEx writes them, is kept as it is.
fn marked(surface: &str, kind: &str, before_root: bool) -> String {
    if surface.starts_with(['-', '=', '~', '<']) || surface.ends_with(['-', '=', '~', '>']) {
        return surface.to_string();
    }
    match kind {
        "prefix" => format!("{surface}-"),
        "suffix" => format!("-{surface}"),
        "infix" => format!("<{surface}>"),
        "clitic" if before_root => format!("{surface}="),
        "clitic" => format!("={surface}"),
        "reduplication" if before_root => format!("{surface}~"),
        "reduplication" => format!("~{surface}"),
        _ => surface.to_string(),
    }
}

fn unique(base: &str, used: &mut HashSet<String>) -> String {
    let mut candidate = base.to_string();
    let mut n = 1;
    while !used.insert(candidate.clone()) {
        n += 1;
        candidate = format!("{base}-{n}");
    }
    candidate
}

/// `YYYY-MM-DDThh:mm:ssZ` from epoch millis (Howard Hinnant's civil-from-days).
fn iso_date(ms: u64) -> String {
    let secs = ms / 1000;
    let (days, rem) = ((secs / 86400) as i64, secs % 86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}
