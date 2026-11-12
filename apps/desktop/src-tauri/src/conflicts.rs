//! The Conflicts panel's queue (roadmap item 24): every field two devices
//! wrote without seeing each other's value, described in words, with its
//! versions side by side. Choosing one, or typing another value, is a
//! resolving write that closes the conflict on every device (DATA-MODEL §5.3).

use serde::Serialize;
use wugs_core::{OpenConflict, Project, Version};
use wugs_model::{looks_like_id, Value};

use crate::session::{project, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionView {
    /// The value as JSON, sent back unchanged to pick this version.
    pub raw: String,
    /// The value in words: text as is, a reference by what it names.
    pub label: String,
    /// Wall-clock time of the write, from its HLC (milliseconds).
    pub at_ms: u64,
    /// Written on this device.
    pub mine: bool,
    /// The write that currently wins (newest by HLC).
    pub winner: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictView {
    pub target: String,
    pub field: String,
    /// "Gloss of «ler» in «köpekler»", "Transcription of a segment"…
    pub title: String,
    /// Where it is: the text's title, "Lexicon", "Project settings".
    pub place: String,
    /// The segment to open, when the field belongs to a text.
    pub text_id: Option<String>,
    pub segment_id: Option<String>,
    /// Typing a new value makes sense (a free-text field, not a reference, a
    /// list or a fixed vocabulary).
    pub editable: bool,
    /// The field's open versions (HLCs) as listed; sent back with a choice,
    /// so a choice made on an out-of-date list is refused.
    pub heads: Vec<String>,
    pub versions: Vec<VersionView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictsResolved {
    pub conflicts: Vec<ConflictView>,
    pub notes: Vec<String>,
}

fn s(p: &Project, id: &str, field: &str) -> String {
    p.proj.field_str(id, field).ok().flatten().unwrap_or_default()
}

fn kind(p: &Project, id: &str) -> String {
    p.proj.entity_kind(id).ok().flatten().unwrap_or_default()
}

fn clip(text: &str, n: usize) -> String {
    let t: String = text.chars().take(n).collect();
    if text.chars().count() > n { format!("{t}…") } else { t }
}

/// An entity in a few words.
fn name(p: &Project, id: &str) -> String {
    match kind(p, id).as_str() {
        "lexeme" => format!("«{}»", s(p, id, "lemma")),
        // A sense by its entry only: its gloss may be the very thing in conflict.
        "sense" => format!("«{}»", s(p, &s(p, id, "lexeme"), "lemma")),
        "token" => format!("«{}»", s(p, id, "form")),
        "morph" => format!("«{}»", s(p, id, "surface")),
        "segment" => format!("“{}”", clip(&s(p, id, "content"), 40)),
        "text" => format!("«{}»", s(p, id, "title")),
        "domain" => format!("{} {}", s(p, id, "code"), s(p, id, "label")),
        "concept" => format!("concept {} ‘{}’", s(p, id, "number"), s(p, id, "gloss")),
        "tier" => format!("tier «{}»", s(p, id, "name")),
        "source" => format!("«{}»", s(p, id, "label")),
        k => k.to_string(),
    }
}

fn field_name(field: &str) -> String {
    if let Some(tag) = field.strip_prefix("gloss:") {
        return format!("Gloss ({tag})");
    }
    match field {
        "content" => "Transcription",
        "gloss" => "Gloss",
        "sense" => "Sense link",
        "form" => "Spelling",
        "surface" => "Morph",
        "morphs" => "Word breakdown",
        "tokens" => "Words",
        "lemma" => "Lemma",
        "citation" => "Citation form",
        "pos-of-speech" => "Category",
        "homograph" => "Homograph number",
        "definition" => "Definition",
        "usage" => "Usage note",
        "title" => "Title",
        "label" => "Label",
        "start" => "Start time",
        "end" => "End time",
        "access" => "Access",
        "source" => "Recording",
        "skipped" => "Not applicable",
        "pos" => "Position",
        "type" => "Morph type",
        "skip-note" => "Not-applicable reason",
        other => return other.to_string(),
    }
    .to_string()
}

/// A value in words.
fn label(p: &Project, v: &Value) -> String {
    match v {
        Value::Null => "(empty)".into(),
        Value::Str(t) if t.is_empty() => "(empty)".into(),
        // A sense as a value is told apart by its gloss, too.
        Value::Str(t) if looks_like_id(t) && kind(p, t) == "sense" => {
            format!("{} ‘{}’", name(p, t), s(p, t, "gloss"))
        }
        Value::Str(t) if looks_like_id(t) && !kind(p, t).is_empty() => name(p, t),
        Value::Str(t) => t.clone(),
        Value::Bool(b) => if *b { "yes" } else { "no" }.into(),
        Value::Num(n) => {
            if n.fract() == 0.0 { format!("{}", *n as i64) } else { format!("{n}") }
        }
        Value::Arr(items) => {
            let parts: Vec<String> = items.iter().map(|i| label(p, i).trim_matches(|c| c == '«' || c == '»').to_string()).collect();
            if parts.is_empty() { "(none)".into() } else { parts.join(" · ") }
        }
        other => other.to_json(),
    }
}

/// The text and segment an entity sits in, if any.
fn place(p: &Project, id: &str) -> (String, Option<String>, Option<String>) {
    let mut cur = id.to_string();
    for _ in 0..4 {
        match kind(p, &cur).as_str() {
            "morph" => cur = s(p, &cur, "token"),
            "token" => cur = s(p, &cur, "segment"),
            "segment" => {
                let text = s(p, &cur, "text");
                return (format!("Text «{}»", s(p, &text, "title")), Some(text), Some(cur));
            }
            "text" => return (format!("Text «{}»", s(p, &cur, "title")), Some(cur), None),
            "lexeme" | "sense" | "sense-domain" | "sense-concept" => return ("Lexicon".into(), None, None),
            _ => break,
        }
    }
    ("Project settings".into(), None, None)
}

fn hlc_parts(hlc: &str) -> (u64, String) {
    match wugs_model::Hlc::parse(hlc) {
        Some(h) => (h.millis, h.device),
        None => (0, String::new()),
    }
}

/// Fields a person may type any text into. Others take references, lists,
/// numbers or a fixed vocabulary (`access`, `type`, `pos`), so only an
/// existing version can be kept.
fn free_text(field: &str) -> bool {
    field.starts_with("gloss")
        || matches!(
            field,
            "content" | "definition" | "usage" | "lemma" | "citation" | "pos-of-speech" | "title" | "label" | "form" | "surface" | "skip-note" | "name" | "description" | "notes" | "body"
        )
}

fn view(p: &Project, c: &OpenConflict) -> ConflictView {
    let me = p.device().to_string();
    let setting = c.kind == "setting";
    let version = |v: &Version, winner: bool| {
        let (at_ms, device) = hlc_parts(&v.hlc);
        // A capability pin's value is a provider id: show its name.
        let label = match (&v.value, setting) {
            (Value::Str(id), true) => crate::capability::provider_name(id),
            (v, _) => label(p, v),
        };
        VersionView { raw: v.value.to_json(), label, at_ms, mine: device == me, winner }
    };
    let mut versions = vec![version(&c.winner, true)];
    versions.extend(c.others.iter().rev().map(|v| version(v, false)));
    let (where_, text_id, segment_id) = place(p, &c.target);
    let editable = free_text(&c.field)
        && [&c.winner].into_iter().chain(c.others.iter()).all(|v| match &v.value {
            Value::Str(t) => !looks_like_id(t),
            Value::Null => true,
            _ => false,
        });
    let what = match c.kind.as_str() {
        "segment" => "a segment".to_string(),
        _ => name(p, &c.target),
    };
    let title = match s(p, &c.target, "key").strip_prefix("capability:").and_then(|id| crate::capability::Capability::parse(id).ok()) {
        Some(cap) if setting => format!("This project's model for {}", cap.title().to_lowercase()),
        _ => format!("{} of {what}", field_name(&c.field)),
    };
    ConflictView {
        title,
        target: c.target.clone(),
        field: c.field.clone(),
        place: where_,
        text_id,
        segment_id,
        editable,
        heads: p.proj.heads(&c.target, &c.field).unwrap_or_default(),
        versions,
    }
}

pub fn conflicts_list(session: &mut Session) -> Result<Vec<ConflictView>, String> {
    let p = project(session)?;
    Ok(p.proj.open_conflicts()?.iter().map(|c| view(p, c)).collect())
}

/// Settle a field with `raw` (a version's JSON value, or a typed text as a
/// JSON string). Returns the queue as it is now.
pub fn conflict_resolve(session: &mut Session, target: &str, field: &str, raw: &str, heads: &[String]) -> Result<ConflictsResolved, String> {
    let value = wugs_model::json::parse(raw).map_err(|e| format!("not a value: {e:?}"))?;
    let p = project(session)?;
    let notes = match p.resolve_field(target, field, value, heads) {
        Ok(res) => res.findings.iter().map(|f| format!("{}: {}", f.code, f.message)).collect(),
        // The field moved since the list was drawn: send the fresh list
        // with the reason instead of failing, so the panel can redraw.
        Err(e) if e == wugs_core::conflicts::STALE => vec![e],
        Err(e) => return Err(e),
    };
    Ok(ConflictsResolved { conflicts: p.proj.open_conflicts()?.iter().map(|c| view(p, c)).collect(), notes })
}
