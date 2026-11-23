//! One sentence's annotation as rows to look at and correct (SCROLLS.md §1
//! "Correct where you look", §2 "Trees stay well formed"): what the
//! Sentence panel draws as a dependency tree and a table.
//!
//! A correction is one resolving write of the sentence's `analysis` field,
//! so concurrent fixes of one sentence meet as an ordinary conflict, and a
//! fix settles any open one. Tree problems are reported, never enforced.

use std::collections::HashMap;

use wugs_core::{CommitResult, Project};
use wugs_model::Value;

use crate::{F_ANALYSIS, F_KIND};

/// UD's universal part-of-speech tags.
pub const UPOS: &[&str] = &["ADJ", "ADP", "ADV", "AUX", "CCONJ", "DET", "INTJ", "NOUN", "NUM", "PART", "PRON", "PROPN", "PUNCT", "SCONJ", "SYM", "VERB", "X"];

/// UD's universal dependency relations (subtypes such as `obl:tmod` are written after a colon).
pub const DEPRELS: &[&str] = &[
    "acl", "advcl", "advmod", "amod", "appos", "aux", "case", "cc", "ccomp", "clf", "compound", "conj", "cop", "csubj", "dep", "det", "discourse", "dislocated", "expl", "fixed", "flat", "goeswith", "iobj", "list", "mark", "nmod", "nsubj", "nummod", "obj", "obl", "orphan", "parataxis", "punct", "reparandum", "root", "vocative", "xcomp",
];

/// The columns a person can correct, by the names `analysis` uses.
pub const EDITABLE: &[&str] = &["lemma", "upos", "xpos", "feats", "head", "deprel", "deps", "misc"];

/// One syntactic word of a sentence with its annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub token: String,
    pub form: String,
    pub lemma: String,
    pub upos: String,
    pub xpos: String,
    pub feats: String,
    /// The head as a word number (0 = the root), when it names a word of the sentence.
    pub head: Option<usize>,
    pub deprel: String,
    pub deps: String,
    pub misc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentenceRows {
    pub rows: Vec<Row>,
    /// Whether the sentence has an `analysis` field at all (a treebank's,
    /// or one made by a correction here).
    pub annotated: bool,
    /// Empty nodes kept as they are (their count, for the panel to say so).
    pub empty_nodes: usize,
    /// What is wrong with the tree, in words.
    pub problems: Vec<String>,
}

fn s(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or("").to_string()
}

/// The live tokens of a sentence, in order.
fn tokens(p: &Project, seg: &str) -> Result<Vec<String>, String> {
    Ok(p.proj.members(seg)?.into_iter().filter(|t| !p.proj.is_tombstoned(t).unwrap_or(true)).collect())
}

fn analysis(p: &Project, seg: &str) -> Result<Option<Vec<Value>>, String> {
    Ok(match p.proj.field(seg, F_ANALYSIS)? {
        Some((Value::Arr(rows), _)) => Some(rows),
        _ => None,
    })
}

/// A sentence's words and their annotation, with what is wrong with its tree.
pub fn rows(p: &Project, seg: &str) -> Result<SentenceRows, String> {
    let toks = tokens(p, seg)?;
    let index: HashMap<&str, usize> = toks.iter().enumerate().map(|(i, t)| (t.as_str(), i + 1)).collect();
    let stored = analysis(p, seg)?;
    let by_tok: HashMap<String, &Value> = stored.iter().flatten().filter_map(|r| r.get("tok").and_then(Value::as_str).map(|t| (t.to_string(), r))).collect();
    let mut out = Vec::with_capacity(toks.len());
    let mut dangling = 0;
    for t in &toks {
        let r = by_tok.get(t).copied();
        let head_raw = s(r.and_then(|r| r.get("head")));
        let head = match head_raw.as_str() {
            "" => None,
            "0" => Some(0),
            id => {
                let h = index.get(id).copied();
                if h.is_none() {
                    dangling += 1;
                }
                h
            }
        };
        let mut upos = s(r.and_then(|r| r.get("upos")));
        if upos.is_empty() && p.proj.field_str(t, F_KIND)?.as_deref() == Some("punct") {
            upos = "PUNCT".into();
        }
        out.push(Row {
            token: t.clone(),
            form: p.proj.field_str(t, "form")?.unwrap_or_default(),
            lemma: s(r.and_then(|r| r.get("lemma"))),
            upos,
            xpos: s(r.and_then(|r| r.get("xpos"))),
            feats: s(r.and_then(|r| r.get("feats"))),
            head,
            deprel: s(r.and_then(|r| r.get("deprel"))),
            deps: s(r.and_then(|r| r.get("deps"))),
            misc: s(r.and_then(|r| r.get("misc"))),
        });
    }
    let empty_nodes = stored.iter().flatten().filter(|r| r.get("tok").is_none()).count();
    let mut problems = problems(&out.iter().map(|r| r.head).collect::<Vec<_>>());
    if dangling > 0 {
        problems.push(format!("{dangling} {} at a word no longer in the sentence.", if dangling == 1 { "head points" } else { "heads point" }));
    }
    Ok(SentenceRows { rows: out, annotated: stored.is_some(), empty_nodes, problems })
}

/// What is wrong with a tree given each word's head (word numbers, 0 = root,
/// `None` = not given): no root or more than one, and words whose heads
/// lead round in a cycle. A sentence with no heads at all has no problems.
pub fn problems(heads: &[Option<usize>]) -> Vec<String> {
    let mut out = Vec::new();
    if heads.iter().all(Option::is_none) {
        return out;
    }
    let roots = heads.iter().filter(|h| **h == Some(0)).count();
    if roots == 0 {
        out.push("No word is the root (head 0).".into());
    } else if roots > 1 {
        out.push(format!("{roots} words are the root; a tree has one."));
    }
    let mut in_cycle = vec![false; heads.len()];
    for start in 0..heads.len() {
        let mut seen = vec![false; heads.len()];
        let mut at = start;
        loop {
            if seen[at] {
                if at == start {
                    in_cycle[start] = true;
                }
                break;
            }
            seen[at] = true;
            match heads[at] {
                Some(h) if h >= 1 && h <= heads.len() => at = h - 1,
                _ => break,
            }
        }
    }
    let cycle: Vec<String> = in_cycle.iter().enumerate().filter(|(_, c)| **c).map(|(i, _)| (i + 1).to_string()).collect();
    if !cycle.is_empty() {
        out.push(format!("Words {} head each other in a circle; one of them must hang from a word outside it.", cycle.join(", ")));
    }
    out
}

/// Correct one cell of one word: `column` is one of [`EDITABLE`]; for
/// `head`, `value` is a word number (`0` for the root, empty to clear).
/// One resolving write of the whole `analysis`; a sentence with none gets
/// one, with a row for every word. Values are checked: UPOS must be a UD
/// tag, FEATS `Name=Value` pairs joined by `|`, and no cell holds a tab or
/// a line break.
pub fn set_cell(p: &mut Project, text: &str, seg: &str, token: &str, column: &str, value: &str) -> Result<CommitResult, String> {
    if !EDITABLE.contains(&column) {
        return Err(format!("{column} is not a column the Sentence panel edits"));
    }
    let toks = tokens(p, seg)?;
    if !toks.iter().any(|t| t == token) {
        return Err("that word is no longer in the sentence; reload it".into());
    }
    let value = value.trim();
    if value.contains(['\t', '\n', '\r']) {
        return Err("a cell cannot hold a tab or a line break".into());
    }
    let value = match column {
        "upos" if !value.is_empty() && !UPOS.contains(&value.to_ascii_uppercase().as_str()) => return Err(format!("«{value}» is not a UD part-of-speech tag; use one of {}", UPOS.join(", "))),
        "upos" => value.to_ascii_uppercase(),
        "feats" if !value.is_empty() && !value.split('|').all(|f| f.split_once('=').is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())) => {
            return Err(format!("«{value}» is not features in UD form: Name=Value pairs joined by |, such as Number=Plur|Case=Nom"))
        }
        "head" if value.is_empty() || value == "_" => String::new(),
        "head" => {
            let n: usize = value.parse().map_err(|_| format!("«{value}» is not a word number"))?;
            if n == 0 {
                "0".into()
            } else {
                let at = toks.iter().position(|t| t == token).unwrap_or(0) + 1;
                if n == at {
                    return Err("a word cannot be its own head".into());
                }
                toks.get(n - 1).cloned().ok_or_else(|| format!("the sentence has {} words, so there is no word {n}", toks.len()))?
            }
        }
        "lemma" => p.proj.nfc(value),
        _ => value.to_string(),
    };
    let stored = analysis(p, seg)?.unwrap_or_default();
    // Rows in token order, the empty nodes back after the word they followed.
    let mut by_tok: HashMap<String, Value> = HashMap::new();
    let mut empties_after: HashMap<Option<String>, Vec<Value>> = HashMap::new();
    let mut last: Option<String> = None;
    for r in stored {
        match r.get("tok").and_then(Value::as_str).map(str::to_string) {
            Some(t) => {
                last = Some(t.clone());
                by_tok.insert(t, r);
            }
            None => empties_after.entry(last.clone()).or_default().push(r),
        }
    }
    let mut rows = empties_after.remove(&None).unwrap_or_default();
    for t in &toks {
        let mut row = match by_tok.remove(t) {
            Some(Value::Obj(m)) => m,
            _ => std::iter::once(("tok".to_string(), Value::str(t))).collect(),
        };
        if t == token {
            if value.is_empty() {
                row.remove(column);
            } else {
                row.insert(column.to_string(), Value::Str(value.clone()));
            }
        }
        rows.push(Value::Obj(row));
        rows.extend(empties_after.remove(&Some(t.clone())).unwrap_or_default());
    }
    // Empty nodes after words that are gone go at the end, not lost.
    for (_, rest) in empties_after {
        rows.extend(rest);
    }
    let mut t = p.intent("correct-analysis").resolving();
    t.set("segment", seg, text, F_ANALYSIS, Value::Arr(rows))?;
    t.commit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_problems_in_words() {
        assert!(problems(&[None, None]).is_empty(), "no heads, nothing to say");
        assert!(problems(&[Some(2), Some(0), Some(2)]).is_empty());
        assert_eq!(problems(&[Some(0), Some(0)]), ["2 words are the root; a tree has one."]);
        let p = problems(&[Some(2), Some(1), Some(0)]);
        assert!(p.iter().any(|m| m.contains("Words 1, 2 head each other")), "{p:?}");
        assert!(problems(&[Some(2), Some(1)]).iter().any(|m| m.contains("No word is the root")));
    }
}
