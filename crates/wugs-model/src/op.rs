//! The op envelope (DATA-MODEL.md §2.2–2.3). One JSON object per log line.

use crate::json::{parse, Value};
use std::collections::BTreeMap;

pub const VERBS: [&str; 5] = ["create", "set", "delete", "restore", "text-edit"];

#[derive(Debug, Clone, PartialEq)]
pub struct Op {
    pub id: String,
    pub hlc: String,
    pub txn: String,
    pub intent: Option<String>,
    pub actor: String,
    pub device: String,
    pub session: Option<String>,
    pub scope: String,
    pub schema: u32,
    /// one of VERBS ("op" in the wire format)
    pub verb: String,
    pub entity: String,
    pub target: String,
    pub field: Option<String>,
    pub value: Value,
    /// HLC of the field value this write overwrote: the winner it saw.
    pub base: Option<String>,
    /// HLCs of every open version of the field the writer had (winner plus
    /// unresolved conflict heads), like a merge commit's parents. None means
    /// `[base]` (or nothing when base is null): old ops, and every write that
    /// saw no conflict, leave it out.
    pub heads: Option<Vec<String>>,
}

impl Op {
    /// The field versions this write supersedes: `heads`, else `[base]`.
    pub fn refs(&self) -> Vec<&str> {
        match &self.heads {
            Some(h) => h.iter().map(String::as_str).collect(),
            None => self.base.iter().map(String::as_str).collect(),
        }
    }

    /// Record the open heads the writer saw, leaving `heads` out when they
    /// say no more than `base` does.
    pub fn set_heads(&mut self, mut heads: Vec<String>) {
        heads.sort();
        heads.dedup();
        let implied: Vec<String> = self.base.iter().cloned().collect();
        self.heads = if heads == implied { None } else { Some(heads) };
    }

    pub fn to_json(&self) -> String {
        let mut m: BTreeMap<String, Value> = BTreeMap::new();
        m.insert("id".into(), Value::str(&self.id));
        m.insert("hlc".into(), Value::str(&self.hlc));
        m.insert("txn".into(), Value::str(&self.txn));
        if let Some(i) = &self.intent {
            m.insert("intent".into(), Value::str(i));
        }
        m.insert("actor".into(), Value::str(&self.actor));
        m.insert("device".into(), Value::str(&self.device));
        if let Some(s) = &self.session {
            m.insert("session".into(), Value::str(s));
        }
        m.insert("scope".into(), Value::str(&self.scope));
        m.insert("schema".into(), Value::Num(self.schema as f64));
        m.insert("op".into(), Value::str(&self.verb));
        m.insert("entity".into(), Value::str(&self.entity));
        m.insert("target".into(), Value::str(&self.target));
        if let Some(f) = &self.field {
            m.insert("field".into(), Value::str(f));
        }
        if self.value != Value::Null {
            m.insert("value".into(), self.value.clone());
        }
        if let Some(b) = &self.base {
            m.insert("base".into(), Value::str(b));
        }
        if let Some(h) = &self.heads {
            m.insert("heads".into(), Value::Arr(h.iter().map(|x| Value::str(x)).collect()));
        }
        Value::Obj(m).to_json()
    }

    pub fn from_json(line: &str) -> Result<Op, String> {
        let v = parse(line).map_err(|e| e.to_string())?;
        let req = |k: &str| -> Result<String, String> {
            v.get_str(k)
                .map(str::to_string)
                .ok_or_else(|| format!("op missing field '{k}'"))
        };
        let opt = |k: &str| v.get_str(k).map(str::to_string);
        let verb = req("op")?;
        if !VERBS.contains(&verb.as_str()) {
            return Err(format!("unknown op verb '{verb}'"));
        }
        let heads = match v.get("heads") {
            None | Some(Value::Null) => None,
            Some(Value::Arr(a)) => Some(
                a.iter()
                    .map(|h| h.as_str().map(str::to_string))
                    .collect::<Option<Vec<String>>>()
                    .ok_or("op 'heads' must hold hlc strings")?,
            ),
            Some(_) => return Err("op 'heads' must be an array".into()),
        };
        Ok(Op {
            id: req("id")?,
            hlc: req("hlc")?,
            txn: req("txn")?,
            intent: opt("intent"),
            actor: req("actor")?,
            device: req("device")?,
            session: opt("session"),
            scope: req("scope")?,
            schema: v
                .get("schema")
                .and_then(|s| s.as_f64())
                .ok_or("op missing field 'schema'")? as u32,
            verb,
            entity: req("entity")?,
            target: req("target")?,
            field: opt("field"),
            value: v.get("value").cloned().unwrap_or(Value::Null),
            base: opt("base"),
            heads,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Op {
        Op {
            id: "op_01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
            hlc: "018f3c2a7b40-0003-dvc_01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
            txn: "txn_01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
            intent: Some("split-morph".into()),
            actor: "prs_01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
            device: "dvc_01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
            session: None,
            scope: "txt_01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
            schema: 1,
            verb: "set".into(),
            entity: "morph".into(),
            target: "mor_01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
            field: Some("gloss".into()),
            value: Value::str("PL"),
            base: None,
            heads: None,
        }
    }

    #[test]
    fn roundtrip() {
        let op = sample();
        let line = op.to_json();
        let back = Op::from_json(&line).unwrap();
        assert_eq!(op, back);
        let mut merged = sample();
        merged.base = Some("018f3c2a7b41-0000-dvc_A".into());
        merged.set_heads(vec!["018f3c2a7b42-0000-dvc_B".into(), "018f3c2a7b41-0000-dvc_A".into()]);
        let line = merged.to_json();
        assert!(line.contains(r#""heads":["018f3c2a7b41-0000-dvc_A","018f3c2a7b42-0000-dvc_B"]"#), "{line}");
        assert_eq!(Op::from_json(&line).unwrap(), merged);
    }

    #[test]
    fn heads_default_to_base() {
        // A line written before `heads` existed, and one whose heads equal
        // its base, read the same way.
        let mut op = sample();
        op.base = Some("018f3c2a7b41-0000-dvc_A".into());
        op.set_heads(vec!["018f3c2a7b41-0000-dvc_A".into()]);
        assert_eq!(op.heads, None, "heads equal to [base] are left out");
        let old = Op::from_json(&op.to_json()).unwrap();
        assert_eq!(old.refs(), ["018f3c2a7b41-0000-dvc_A"]);
        op.base = None;
        assert!(op.refs().is_empty());
        let bad = sample().to_json().replace(r#""id":"#, r#""heads":"x","id":"#);
        assert!(Op::from_json(&bad).is_err());
    }

    #[test]
    fn rejects_unknown_verb() {
        let op = sample();
        let line = op.to_json().replace("\"set\"", "\"frobnicate\"");
        assert!(Op::from_json(&line).is_err());
    }
}
