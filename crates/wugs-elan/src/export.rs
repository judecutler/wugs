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

    let mut header = XmlNode::new("HEADER")
        .with_attr("MEDIA_FILE", "")
        .with_attr("TIME_UNITS", "milliseconds")
        .with_child(XmlNode::new("PROPERTY").with_attr("NAME", PROP_TEXT).with_text(text));
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
