use std::collections::{BTreeMap, HashMap};
use std::fmt;

use wugs_core::Project;
use wugs_model::Value;
use wugs_xml::{XmlError, XmlNode};

/// One string in several writing systems, in document order: `(lang, text)`.
pub type MultiText = Vec<(String, String)>;

pub const F_ORIGIN: &str = "origin";

#[derive(Debug)]
pub enum ImportError {
    Xml(XmlError),
    Format(String),
    Project(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Xml(e) => e.fmt(f),
            ImportError::Format(m) => write!(f, "invalid FLEx file: {m}"),
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

pub fn format_err(msg: impl Into<String>) -> ImportError {
    ImportError::Format(msg.into())
}

/// Element names the importer read but does not carry, with counts, so a
/// user sees exactly what a round trip would lose.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct NotCarried(pub BTreeMap<String, usize>);

impl NotCarried {
    pub fn add(&mut self, what: &str) {
        *self.0.entry(what.to_string()).or_default() += 1;
    }
    pub fn total(&self) -> usize {
        self.0.values().sum()
    }
}

pub fn multi_value(m: &MultiText) -> Value {
    Value::Arr(
        m.iter()
            .map(|(l, t)| Value::Arr(vec![Value::str(l), Value::str(t)]))
            .collect(),
    )
}

pub fn multi_from(v: &Value) -> Option<MultiText> {
    let Value::Arr(items) = v else { return None };
    items
        .iter()
        .map(|pair| match pair {
            Value::Arr(p) if p.len() == 2 => Some((p[0].as_str()?.to_string(), p[1].as_str()?.to_string())),
            _ => None,
        })
        .collect()
}

/// The stored multistring with its first entry replaced by the entity's
/// current semantic value, so edits made in wugs win on export: a cleared
/// value removes the first entry, and a value with nothing stored becomes a
/// single entry in `default_lang`.
pub fn with_primary(stored: Option<MultiText>, primary: Option<String>, default_lang: &str) -> MultiText {
    let mut m = stored.unwrap_or_default();
    match (primary.filter(|p| !p.is_empty()), m.is_empty()) {
        (Some(p), false) => m[0].1 = p,
        (Some(p), true) => m.push((default_lang.to_string(), p)),
        (None, false) => {
            m.remove(0);
        }
        (None, true) => {}
    }
    m
}

pub fn field_value(project: &Project, id: &str, field: &str) -> Result<Option<Value>, String> {
    Ok(project.proj.field(id, field)?.map(|(v, _)| v))
}

pub fn field_multi(project: &Project, id: &str, field: &str) -> Result<Option<MultiText>, String> {
    Ok(field_value(project, id, field)?.as_ref().and_then(multi_from))
}

/// origin -> entity id for every live entity of `kind`.
pub fn origin_index(project: &Project, kind: &str) -> Result<HashMap<String, String>, String> {
    let mut s = project.proj.db.prepare(
        "SELECT f.value, f.target FROM fields f JOIN entities e ON e.id=f.target
         WHERE f.field=?1 AND e.kind=?2 AND e.tombstone=0",
    )?;
    s.bind_text(1, F_ORIGIN)?;
    s.bind_text(2, kind)?;
    let mut out = HashMap::new();
    while s.step()? {
        if let Some(origin) = wugs_model::json::parse(&s.col_text(0)).ok().and_then(|v| v.as_str().map(str::to_string)) {
            out.insert(origin, s.col_text(1));
        }
    }
    Ok(out)
}

/// An origin key resolves to an entity when it is that entity's own id (an
/// export of born-digital data) or some entity's recorded origin.
pub fn resolve_origin(project: &Project, index: &HashMap<String, String>, kind: &str, key: &str) -> Result<Option<String>, String> {
    if project.proj.entity_kind(key)?.as_deref() == Some(kind) && !project.proj.is_tombstoned(key)? {
        return Ok(Some(key.to_string()));
    }
    Ok(index.get(key).cloned())
}

pub fn fnv1a(s: &str) -> u64 {
    s.bytes()
        .fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

pub fn req<'a>(node: &'a XmlNode, attr: &str) -> Result<&'a str, ImportError> {
    node.attr(attr)
        .ok_or_else(|| format_err(format!("<{}> missing {attr}", node.name)))
}

pub fn opt_ms(node: &XmlNode, attr: &str) -> Result<Option<f64>, ImportError> {
    node.attr(attr)
        .map(|v| {
            v.trim()
                .parse::<f64>()
                .map_err(|_| format_err(format!("<{}> has non-numeric {attr} {v:?}", node.name)))
        })
        .transpose()
}


/// Every attribute of an element, so export can re-emit the ones the
/// importer does not interpret (FLEx's `dateModified` drives its merge).
pub fn attrs_value(n: &XmlNode) -> Value {
    Value::Obj(n.attrs.iter().map(|(k, v)| (k.clone(), Value::str(v))).collect())
}

/// Stored attributes, then `set` on top (derived values win), minus `drop`.
pub fn with_attrs(mut n: XmlNode, stored: Option<Value>, set: &[(&str, Option<String>)]) -> XmlNode {
    let mut attrs: Vec<(String, String)> = match stored {
        Some(Value::Obj(m)) => m.into_iter().filter_map(|(k, v)| Some((k, v.as_str()?.to_string()))).collect(),
        _ => Vec::new(),
    };
    for (k, v) in set {
        attrs.retain(|(ak, _)| ak != k);
        if let Some(v) = v {
            attrs.push((k.to_string(), v.clone()));
        }
    }
    n.attrs.extend(attrs);
    n
}

/// A whole element as a value, for subtrees carried verbatim.
pub fn node_value(n: &XmlNode) -> Value {
    Value::obj(vec![
        ("name", Value::str(&n.name)),
        ("attrs", attrs_value(n)),
        ("text", Value::str(&n.text)),
        ("children", Value::Arr(n.children.iter().map(node_value).collect())),
    ])
}

pub fn value_node(v: &Value) -> Option<XmlNode> {
    let mut n = with_attrs(XmlNode::new(v.get_str("name")?), v.get("attrs").cloned(), &[]);
    n.text = v.get_str("text").unwrap_or("").to_string();
    if let Some(Value::Arr(cs)) = v.get("children") {
        n.children = cs.iter().filter_map(value_node).collect();
    }
    Some(n)
}

/// A stable GUID-shaped key for an entity that has no FLEx guid, so our own
/// exports re-import onto the same entity and FLEx still accepts the file.
pub fn derived_guid(id: &str) -> String {
    let a = fnv1a(id);
    let b = fnv1a(&format!("{id}#wugs"));
    let h = format!("{a:016x}{b:016x}");
    format!(
        "{}-{}-8{}-a{}-{}",
        &h[0..8],
        &h[8..12],
        &h[13..16],
        &h[17..20],
        &h[20..32]
    )
}
