use std::collections::{HashMap, HashSet};
use std::fmt;

use wugs_core::{Project, SCOPE_CONFIG};
use wugs_model::{pos_seq, Value};

use crate::mapping::*;
use crate::xml::{self, XmlError, XmlNode};

/// Ops per commit when a single tier is large.
const CHUNK_OPS: usize = 400;

#[derive(Debug)]
pub enum ImportError {
    Xml(XmlError),
    Eaf(String),
    Project(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Xml(e) => e.fmt(f),
            ImportError::Eaf(m) => write!(f, "invalid EAF: {m}"),
            ImportError::Project(m) => write!(f, "project error: {m}"),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<XmlError> for ImportError {
    fn from(e: XmlError) -> Self {
        ImportError::Xml(e)
    }
}

impl From<String> for ImportError {
    fn from(e: String) -> Self {
        ImportError::Project(e)
    }
}

#[derive(Debug, Default)]
pub struct ImportReport {
    pub text: String,
    pub text_created: bool,
    pub tiers_created: usize,
    pub tiers_skipped: usize,
    pub segments_created: usize,
    pub segments_skipped: usize,
    pub commits: usize,
    pub findings: usize,
}

struct EafDoc {
    key: String,
    title: Option<String>,
    tiers: Vec<TierIn>,
}

struct TierIn {
    id: String,
    order: usize,
    lt_ref: Option<String>,
    constraints: Option<String>,
    kind: &'static str,
    parent: Option<String>,
    participant: Option<String>,
    anns: Vec<AnnIn>,
}

struct AnnIn {
    id: String,
    content: String,
    shape: Shape,
}

enum Shape {
    Aligned { start: Option<f64>, end: Option<f64> },
    Ref { parent: String },
}

/// Import one EAF document as a new text. Re-importing a file already in the
/// project (same file, or an export of a text in this project) creates nothing:
/// existing tiers and segments are matched by origin and skipped, not updated.
pub fn import_eaf(
    project: &mut Project,
    eaf: &str,
    title_fallback: &str,
) -> Result<ImportReport, ImportError> {
    let doc = parse_eaf(eaf)?;
    let mut report = ImportReport::default();

    let title = doc.title.as_deref().unwrap_or(title_fallback);
    let text = match find_text(project, &doc.key)? {
        Some(id) => id,
        None => {
            let mut t = project.intent("import-elan");
            let id = t.create_scope_root(
                "text",
                "txt",
                vec![
                    ("title", Value::str(title)),
                    (F_ORIGIN, Value::str(&doc.key)),
                    ("kind", Value::str("unspecified")),
                ],
            );
            t.commit()?;
            report.text_created = true;
            report.commits += 1;
            id
        }
    };
    report.text = text.clone();

    let mut tier_ids = origin_map(project, "x.elan.text", &text, "tier")?;
    let mut seg_ids = origin_map(project, "text", &text, "segment")?;

    // Tiers live in SCOPE_CONFIG and segments in the text's scope. Each txn
    // stays in one scope, or scope-filtered sync would deliver half of it.
    for tier in &doc.tiers {
        let tier_id = match tier_ids.get(&tier.id) {
            Some(id) => {
                report.tiers_skipped += 1;
                id.clone()
            }
            None => {
                let mut fields = vec![
                    ("name", Value::str(&tier.id)),
                    ("kind", Value::str(tier.kind)),
                    ("participant", Value::Bool(tier.participant.is_some())),
                    (F_ORIGIN, Value::str(&tier.id)),
                    (F_TIER_TEXT, Value::str(&text)),
                    (F_TIER_ORDER, Value::Num(tier.order as f64)),
                ];
                if let Some(p) = &tier.parent {
                    fields.push(("parent", Value::str(&tier_ids[p])));
                }
                for (key, val) in [
                    (F_LING_TYPE, &tier.lt_ref),
                    (F_CONSTRAINTS, &tier.constraints),
                    (F_PARTICIPANT, &tier.participant),
                ] {
                    if let Some(v) = val {
                        fields.push((key, Value::str(v)));
                    }
                }
                let mut t = project.intent("import-elan");
                let id = t.create("tier", "tir", SCOPE_CONFIG, fields);
                let res = t.commit()?;
                report.commits += 1;
                report.findings += res.findings.len();
                tier_ids.insert(tier.id.clone(), id.clone());
                report.tiers_created += 1;
                id
            }
        };

        let mut txn = project.intent("import-elan");
        let mut pending = 0;
        for (i, ann) in tier.anns.iter().enumerate() {
            if seg_ids.contains_key(&ann.id) {
                report.segments_skipped += 1;
                continue;
            }
            let mut fields = vec![
                ("text", Value::str(&text)),
                ("tier", Value::str(&tier_id)),
                ("pos", Value::str(&pos_seq(i))),
                ("content", Value::str(&ann.content)),
                (F_ORIGIN, Value::str(&ann.id)),
            ];
            match &ann.shape {
                Shape::Aligned { start, end } => {
                    if let Some(s) = start {
                        fields.push(("start", Value::Num(*s)));
                    }
                    if let Some(e) = end {
                        fields.push(("end", Value::Num(*e)));
                    }
                }
                Shape::Ref { parent } => {
                    let pid = seg_ids.get(parent).ok_or_else(|| {
                        ImportError::Eaf(format!(
                            "annotation {} refers to {parent}, which is not in the parent tier",
                            ann.id
                        ))
                    })?;
                    fields.push(("parent", Value::str(pid)));
                }
            }
            let id = txn.create("segment", "seg", &text, fields);
            seg_ids.insert(ann.id.clone(), id);
            report.segments_created += 1;
            pending += 1;
            if pending >= CHUNK_OPS {
                let res = txn.commit()?;
                report.commits += 1;
                report.findings += res.findings.len();
                txn = project.intent("import-elan");
                pending = 0;
            }
        }
        let res = txn.commit()?;
        report.commits += 1;
        report.findings += res.findings.len();
    }
    Ok(report)
}

fn find_text(project: &Project, key: &str) -> Result<Option<String>, String> {
    let db = &project.proj.db;
    if project.proj.entity_kind(key)?.as_deref() == Some("text") {
        return Ok(Some(key.to_string()));
    }
    let mut s = db.prepare(
        "SELECT f.target FROM fields f JOIN entities e ON e.id=f.target
         WHERE f.field=?1 AND f.value=?2 AND e.kind='text' AND e.tombstone=0 LIMIT 1",
    )?;
    s.bind_text(1, F_ORIGIN)?;
    s.bind_text(2, &Value::str(key).to_json())?;
    Ok(if s.step()? { Some(s.col_text(0)) } else { None })
}

fn origin_map(
    project: &Project,
    ref_field: &str,
    text: &str,
    kind: &str,
) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();
    for id in project.proj.children_by_ref(ref_field, text, kind)? {
        if let Some(origin) = project.proj.field_str(&id, F_ORIGIN)? {
            out.insert(origin, id);
        }
    }
    Ok(out)
}

fn fnv1a(s: &str) -> u64 {
    s.bytes()
        .fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

fn parse_eaf(input: &str) -> Result<EafDoc, ImportError> {
    let root = xml::parse(input)?;
    if root.name != "ANNOTATION_DOCUMENT" {
        return Err(ImportError::Eaf(format!("root element is <{}>", root.name)));
    }
    let prop = |name: &str| {
        root.child("HEADER")?
            .children_named("PROPERTY")
            .find(|p| p.attr("NAME") == Some(name))
            .map(|p| p.text.clone())
    };
    let key = prop(PROP_TEXT).unwrap_or_else(|| format!("eaf-fnv:{:016x}", fnv1a(input)));

    let mut slots: HashMap<&str, Option<f64>> = HashMap::new();
    if let Some(order) = root.child("TIME_ORDER") {
        for s in order.children_named("TIME_SLOT") {
            let id = req(s, "TIME_SLOT_ID")?;
            let value = match s.attr("TIME_VALUE") {
                Some(v) => Some(v.trim().parse::<f64>().map_err(|_| {
                    ImportError::Eaf(format!("slot {id} has non-numeric TIME_VALUE {v:?}"))
                })?),
                None => None,
            };
            slots.insert(id, value);
        }
    }

    let mut types: HashMap<&str, (bool, Option<&str>)> = HashMap::new();
    for lt in root.children_named("LINGUISTIC_TYPE") {
        types.insert(
            req(lt, "LINGUISTIC_TYPE_ID")?,
            (lt.attr("TIME_ALIGNABLE") == Some("true"), lt.attr("CONSTRAINTS")),
        );
    }

    let slot = |ann: &XmlNode, attr: &str| -> Result<Option<f64>, ImportError> {
        let id = req(ann, attr)?;
        slots
            .get(id)
            .copied()
            .ok_or_else(|| ImportError::Eaf(format!("unknown time slot {id}")))
    };

    let mut tiers = Vec::new();
    for (order, t) in root.children_named("TIER").enumerate() {
        let id = req(t, "TIER_ID")?.to_string();
        let lt_ref = t.attr("LINGUISTIC_TYPE_REF");
        let (alignable, constraints) = lt_ref
            .and_then(|r| types.get(r).copied())
            .unwrap_or((true, None));
        let mut anns = Vec::new();
        for a in t.children_named("ANNOTATION") {
            let inner = a
                .children
                .first()
                .ok_or_else(|| ImportError::Eaf(format!("empty ANNOTATION in tier {id}")))?;
            let content = inner
                .child("ANNOTATION_VALUE")
                .map(|v| v.text.clone())
                .unwrap_or_default();
            let (ann_id, shape) = match inner.name.as_str() {
                "ALIGNABLE_ANNOTATION" => (
                    req(inner, "ANNOTATION_ID")?,
                    Shape::Aligned {
                        start: slot(inner, "TIME_SLOT_REF1")?,
                        end: slot(inner, "TIME_SLOT_REF2")?,
                    },
                ),
                "REF_ANNOTATION" => (
                    req(inner, "ANNOTATION_ID")?,
                    Shape::Ref {
                        parent: req(inner, "ANNOTATION_REF")?.to_string(),
                    },
                ),
                other => return Err(ImportError::Eaf(format!("unexpected <{other}> in tier {id}"))),
            };
            anns.push(AnnIn {
                id: ann_id.to_string(),
                content,
                shape,
            });
        }
        tiers.push(TierIn {
            id,
            order,
            lt_ref: lt_ref.map(str::to_string),
            constraints: constraints.map(str::to_string),
            kind: kind_for(alignable, constraints),
            parent: t.attr("PARENT_REF").map(str::to_string),
            participant: t.attr("PARTICIPANT").map(str::to_string),
            anns,
        });
    }

    let mut seen = HashSet::new();
    for t in &tiers {
        if !seen.insert(t.id.as_str()) {
            return Err(ImportError::Eaf(format!("duplicate TIER_ID {}", t.id)));
        }
    }
    let mut ann_seen = HashSet::new();
    for a in tiers.iter().flat_map(|t| &t.anns) {
        if !ann_seen.insert(a.id.as_str()) {
            return Err(ImportError::Eaf(format!("duplicate ANNOTATION_ID {}", a.id)));
        }
    }
    for t in &tiers {
        if let Some(p) = &t.parent {
            if !seen.contains(p.as_str()) {
                return Err(ImportError::Eaf(format!("tier {} has unknown PARENT_REF {p}", t.id)));
            }
        }
    }

    Ok(EafDoc {
        key,
        title: prop(PROP_TITLE),
        tiers: parents_first(tiers)?,
    })
}

/// Parent tiers must exist before children so that PARENT_REF and
/// ANNOTATION_REF resolve to entity ids; document order otherwise.
fn parents_first(mut pending: Vec<TierIn>) -> Result<Vec<TierIn>, ImportError> {
    let mut done: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(pending.len());
    while !pending.is_empty() {
        let before = pending.len();
        let mut rest = Vec::new();
        for t in pending {
            if t.parent.as_ref().is_none_or(|p| done.contains(p)) {
                done.insert(t.id.clone());
                out.push(t);
            } else {
                rest.push(t);
            }
        }
        if rest.len() == before {
            return Err(ImportError::Eaf("PARENT_REF cycle between tiers".into()));
        }
        pending = rest;
    }
    Ok(out)
}

fn req<'a>(node: &'a XmlNode, attr: &str) -> Result<&'a str, ImportError> {
    node.attr(attr)
        .ok_or_else(|| ImportError::Eaf(format!("<{}> missing {attr}", node.name)))
}
