//! Machine tags and trees for segments that are already in the project: a
//! tagger's CoNLL-U (UDPipe, Stanza…) given one sentence per segment, in
//! order. A segment with no words gets the tagger's tokens; a segment whose
//! words read as the tagger's (punctuation aside) keeps its tokens and gets
//! rows keyed to them, so glosses and links stay; a segment with an
//! `analysis` already (imported, or corrected by a person) is left alone.

use std::collections::HashMap;

use wugs_core::Project;
use wugs_model::{pos_seq, Value};

use crate::format::{self, Line, Sentence};
use crate::{columns, row, F_ANALYSIS, F_KIND, F_MWT};

/// The field that records which tagger wrote a segment's analysis.
pub const F_TAGGED_BY: &str = "x.morphosyntax";

/// The intent a tagger's analyses are written under.
pub const MACHINE_INTENT: &str = "machine-annotate";

/// The field that records the machine write `op` of `analysis`: the
/// tagger and model, keyed by the op so a mark belongs to that one write
/// (SCROLLS.md §2, "Checked, not just unedited"), as speech assist keys its
/// records (`wugs_core::assist::record_field`).
pub fn record_field(op: &str) -> String {
    format!("x.tagged:{op}")
}

/// The tagger behind a sentence's analysis while a person has not looked at
/// it: set while any open version of `analysis` is a tagger's write. A
/// correction, or Mark checked, writes `analysis` as a person and names every
/// open version, which clears it.
pub fn unchecked(p: &Project, segment: &str) -> Result<Option<String>, String> {
    for op in p.proj.open_version_ops(segment, F_ANALYSIS)? {
        if let Some((by, _)) = p.proj.field(segment, &record_field(&op))? {
            return Ok(Some(by.as_str().unwrap_or_default().to_string()));
        }
    }
    Ok(None)
}

/// Mark sentences' machine tags as checked without changing them: a
/// person's resolving write of each one's analysis as it reads now, in one
/// txn. Sentences with nothing unchecked are left out; returns how many were
/// marked.
pub fn mark_checked(p: &mut Project, text: &str, segments: &[String]) -> Result<usize, String> {
    let mut now: Vec<(String, Value)> = Vec::new();
    for seg in segments {
        if p.proj.is_tombstoned(seg)? || p.proj.field_str(seg, "text")?.as_deref() != Some(text) || unchecked(p, seg)?.is_none() {
            continue;
        }
        if let Some((value, _)) = p.proj.field(seg, F_ANALYSIS)? {
            now.push((seg.clone(), value));
        }
    }
    if now.is_empty() {
        return Ok(0);
    }
    let mut t = p.intent("check-analysis").resolving();
    for (seg, value) in &now {
        t.set("segment", seg, text, F_ANALYSIS, value.clone())?;
    }
    t.commit()?;
    Ok(now.len())
}

/// How far a text's tagging is checked: sentences with an analysis, and
/// those of them still a tagger's alone, in order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CheckStatus {
    pub analysed: usize,
    pub unchecked: Vec<String>,
}

pub fn check_status(p: &Project, segments: &[String]) -> Result<CheckStatus, String> {
    let mut st = CheckStatus::default();
    for seg in segments {
        if p.proj.is_tombstoned(seg)? || p.proj.field(seg, F_ANALYSIS)?.is_none() {
            continue;
        }
        st.analysed += 1;
        if unchecked(p, seg)?.is_some() {
            st.unchecked.push(seg.clone());
        }
    }
    Ok(st)
}

/// What a tagger's output did to the segments it was given.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Annotated {
    /// Segments that got an analysis.
    pub annotated: usize,
    /// Of those, segments that had no words and got the tagger's tokens.
    pub tokenized: usize,
    /// Segments left alone because they have an analysis already.
    pub kept: usize,
    /// Segments left alone because their words are split differently from the tagger's.
    pub differ: usize,
    /// Words tagged.
    pub words: usize,
}

/// Write each sentence's analysis on its segment in one plain txn (a
/// machine's suggestion, which closes no one's version), recording
/// `provenance` (the tagger and model) in [`F_TAGGED_BY`].
pub fn annotate(p: &mut Project, text: &str, items: &[(String, Sentence)], provenance: &str) -> Result<Annotated, String> {
    let mut report = Annotated::default();
    // Worked out first; the txn only writes.
    let mut writes: Vec<(String, Vec<(&'static str, Value)>, Vec<(String, String, bool)>)> = Vec::new();
    for (seg, s) in items {
        if p.proj.field(seg, F_ANALYSIS)?.is_some() {
            report.kept += 1;
            continue;
        }
        let words: Vec<(u32, String, bool)> = s.words().map(|(id, c)| (id, p.proj.nfc(&c[format::FORM]), c[format::UPOS] == "PUNCT")).collect();
        let members: Vec<String> = p.proj.members(seg)?.into_iter().filter(|t| !p.proj.is_tombstoned(t).unwrap_or(true)).collect();
        let mut tok_of: HashMap<u32, String> = HashMap::new();
        let mut new_tokens: Vec<(String, String, bool)> = Vec::new();
        if members.is_empty() {
            for (id, form, punct) in &words {
                let tok = wugs_model::new_id("tok");
                tok_of.insert(*id, tok.clone());
                new_tokens.push((tok, form.clone(), *punct));
            }
        } else {
            let forms: Vec<String> = members.iter().map(|t| p.proj.field_str(t, "form").map(|f| p.proj.nfc(&f.unwrap_or_default()))).collect::<Result<_, _>>()?;
            let all: Vec<&(u32, String, bool)> = words.iter().collect();
            let content: Vec<&(u32, String, bool)> = words.iter().filter(|w| !w.2).collect();
            // Every word lines up, or (wugs's own tokens leave punctuation out) every word but punctuation.
            let lined = [all, content].into_iter().find(|ws| ws.len() == forms.len() && ws.iter().zip(&forms).all(|(w, f)| w.1 == *f));
            let Some(lined) = lined else {
                report.differ += 1;
                continue;
            };
            for (w, tok) in lined.iter().zip(&members) {
                tok_of.insert(w.0, tok.clone());
            }
        }
        let mut rows = Vec::new();
        let mut ranges = Vec::new();
        for line in &s.lines {
            match line {
                Line::Word { id, cols } => {
                    let Some(tok) = tok_of.get(id) else { continue };
                    let mut r = row(Some(tok), None, cols, &tok_of, p);
                    // A head among the words left out (punctuation wugs never split off) is dropped, never kept as a bare number.
                    if let Value::Obj(m) = &mut r {
                        if m.get("head").and_then(Value::as_str).is_some_and(|h| h != "0" && !tok_of.values().any(|t| t == h)) {
                            m.remove("head");
                        }
                    }
                    rows.push(r);
                }
                Line::Empty { .. } => {}
                Line::Range { first, last, cols } => {
                    if let (Some(a), Some(b)) = (tok_of.get(first), tok_of.get(last)) {
                        let mut m = columns(cols, &[], p);
                        m.insert("first".into(), Value::str(a));
                        m.insert("last".into(), Value::str(b));
                        ranges.push(Value::Obj(m));
                    }
                }
            }
        }
        let mut fields: Vec<(&'static str, Value)> = vec![(F_ANALYSIS, Value::Arr(rows)), (F_TAGGED_BY, Value::str(provenance))];
        if !ranges.is_empty() {
            fields.push((F_MWT, Value::Arr(ranges)));
        }
        report.annotated += 1;
        report.words += tok_of.len();
        if !new_tokens.is_empty() {
            report.tokenized += 1;
        }
        writes.push((seg.clone(), fields, new_tokens));
    }
    if writes.is_empty() {
        return Ok(report);
    }
    let mut t = p.intent(MACHINE_INTENT);
    for (seg, fields, tokens) in writes {
        for (field, value) in fields {
            t.set("segment", &seg, text, field, value)?;
            if field == F_ANALYSIS {
                let op = t.last_op_id().expect("the analysis op was just written").to_string();
                t.set("segment", &seg, text, &record_field(&op), Value::str(provenance))?;
            }
        }
        for (wi, (tok, form, punct)) in tokens.iter().enumerate() {
            let mut fields = vec![("segment", Value::str(&seg)), ("pos", Value::str(&pos_seq(wi))), ("form", Value::str(form))];
            if *punct {
                fields.push((F_KIND, Value::str("punct")));
            }
            t.create_with_id("token", tok, text, fields);
        }
    }
    t.commit()?;
    Ok(report)
}
