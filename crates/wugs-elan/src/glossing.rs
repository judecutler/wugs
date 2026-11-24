//! Glossing kept in an ELAN file's own tiers: a word tier under a
//! transcription tier, a morph tier under the words and a gloss tier on the
//! morphs (or on the words), as Toolbox, FLEx and hand-built ELAN templates
//! lay them out. Import brings such tiers in as tiers of segments like any
//! other; [`read_glossing`] turns them into the text's tokens and morphs, so
//! the Gloss panel, the corpus tools and the exports see the analysis, and
//! takes the tiers away (an ELAN export writes the glossing back as
//! `<tier> words`, `<tier> morphs` and `<tier> glosses`).
//!
//! The tiers are found by their structure, never by name: a word tier is a
//! Symbolic_Subdivision, Time_Subdivision or Included_In child of a
//! transcription tier, a morph tier the same under the word tier, and a gloss
//! tier a Symbolic_Association of the morphs (else of the words). When a tier
//! has several association children, one whose name holds `gl` or `ge` is the
//! gloss tier.

use std::collections::HashMap;

use wugs_core::{Project, SCOPE_CONFIG};
use wugs_model::{pos_seq, Value};

use crate::mapping::*;

/// One set of glossing tiers under a transcription tier of a text.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub transcription: String,
    pub words: String,
    pub morphs: Option<String>,
    pub glosses: Option<String>,
    /// The tiers' names, in the same order (transcription, words, morphs, glosses).
    pub names: Vec<String>,
    /// Other association tiers on the words or morphs (a part of speech, a
    /// second gloss): each value is kept on its word or morph as
    /// `x.elan.<tier name>`, and the tier goes with the others.
    pub kept: Vec<(String, String)>,
    /// Any other tier under the words, which goes with them unread.
    pub dropped: Vec<(String, String)>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Read {
    pub sentences: usize,
    pub words: usize,
    pub morphs: usize,
    pub glossed: usize,
    /// Sentences that already had words in wugs, left as they are.
    pub kept: usize,
    /// Annotations whose parent could not be found, left out.
    pub stray: usize,
}

struct TierInfo {
    id: String,
    name: String,
    kind: String,
    constraints: Option<String>,
    parent: Option<String>,
}

fn tiers_of(p: &Project, text: &str) -> Result<Vec<TierInfo>, String> {
    let mut out = Vec::new();
    for id in p.proj.children_by_ref(F_TIER_TEXT, text, "tier")? {
        out.push(TierInfo {
            name: p.proj.field_str(&id, "name")?.unwrap_or_default(),
            kind: p.proj.field_str(&id, "kind")?.unwrap_or_default(),
            constraints: p.proj.field_str(&id, F_CONSTRAINTS)?,
            parent: p.proj.field_str(&id, "parent")?,
            id,
        });
    }
    Ok(out)
}

impl TierInfo {
    /// A tier that divides its parent's annotations into parts.
    fn divides(&self) -> bool {
        self.kind == KIND_SUBDIVISION || matches!(self.constraints.as_deref(), Some("Time_Subdivision" | "Included_In"))
    }
}

/// The glossing tiers of a text, one layout per transcription tier that has a word tier under it.
pub fn layouts(p: &Project, text: &str) -> Result<Vec<Layout>, String> {
    let tiers = tiers_of(p, text)?;
    let tiers = &tiers;
    fn children<'a>(tiers: &'a [TierInfo], of: &'a str) -> impl Iterator<Item = &'a TierInfo> + 'a {
        tiers.iter().filter(move |t| t.parent.as_deref() == Some(of))
    }
    fn gloss_of<'a>(tiers: &'a [TierInfo], of: &'a str) -> Option<&'a TierInfo> {
        let assoc: Vec<&TierInfo> = children(tiers, of).filter(|t| t.kind == KIND_ASSOCIATION).collect();
        let named = |t: &TierInfo| {
            let n = t.name.to_lowercase();
            n.contains("gl") || n.contains("ge")
        };
        assoc.iter().copied().find(|t| named(t)).or(if assoc.len() == 1 { Some(assoc[0]) } else { None })
    }
    let mut out = Vec::new();
    for t in tiers {
        if t.divides() || t.kind == KIND_ASSOCIATION {
            continue;
        }
        let Some(w) = children(tiers, &t.id).find(|c| c.divides()) else { continue };
        let m = children(tiers, &w.id).find(|c| c.divides());
        let g = gloss_of(tiers, m.map_or(&w.id, |m| &m.id));
        let mut names = vec![t.name.clone(), w.name.clone()];
        names.extend(m.map(|m| m.name.clone()));
        names.extend(g.map(|g| g.name.clone()));
        // Every other tier below the words.
        let read: Vec<&str> = [Some(w.id.as_str()), m.map(|m| m.id.as_str()), g.map(|g| g.id.as_str())].into_iter().flatten().collect();
        let (mut kept, mut dropped) = (Vec::new(), Vec::new());
        let mut queue = read.clone();
        while let Some(of) = queue.pop() {
            for c in children(tiers, of) {
                if read.contains(&c.id.as_str()) {
                    continue;
                }
                queue.push(&c.id);
                let on_read = c.parent.as_deref().is_some_and(|p| p == w.id || m.is_some_and(|m| m.id == p));
                if on_read && c.kind == KIND_ASSOCIATION {
                    kept.push((c.id.clone(), c.name.clone()));
                } else {
                    dropped.push((c.id.clone(), c.name.clone()));
                }
            }
        }
        out.push(Layout { transcription: t.id.clone(), words: w.id.clone(), morphs: m.map(|m| m.id.clone()), glosses: g.map(|g| g.id.clone()), names, kept, dropped });
    }
    Ok(out)
}

struct Seg {
    id: String,
    content: String,
    parent: Option<String>,
    start: Option<f64>,
    end: Option<f64>,
}

/// `ni-` a prefix, `-ra` a suffix, `=ma` or `ma=` a clitic, `<um>` an infix, else a root; markers taken off.
fn morph_of(written: &str) -> (String, &'static str) {
    let w = written.trim();
    let kind = if w.starts_with('<') && w.ends_with('>') {
        "infix"
    } else if w.starts_with('=') || w.ends_with('=') {
        "clitic"
    } else if w.ends_with('-') && !w.starts_with('-') {
        "prefix"
    } else if w.starts_with('-') && !w.ends_with('-') {
        "suffix"
    } else {
        "root"
    };
    (w.trim_matches(|c| matches!(c, '-' | '=' | '<' | '>')).to_string(), kind)
}

/// Each segment's children on `tier`, in order: by `parent` when it names
/// one, else the `among` segment whose span holds the child's middle (a
/// Time_Subdivision tier); then by start time, then by place in the file.
fn children_on(by_tier: &HashMap<&str, Vec<&Seg>>, tier: &str, among: &[&Seg], stray: &mut usize) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<(Option<f64>, usize, String)>> = HashMap::new();
    for (i, s) in by_tier.get(tier).into_iter().flatten().enumerate() {
        let owner = s.parent.clone().or_else(|| {
            let mid = (s.start? + s.end?) / 2.0;
            among.iter().find(|o| o.start.is_some_and(|a| a <= mid) && o.end.is_some_and(|b| mid <= b)).map(|o| o.id.clone())
        });
        match owner {
            Some(o) => out.entry(o).or_default().push((s.start, i, s.id.clone())),
            None => *stray += 1,
        }
    }
    out.into_iter()
        .map(|(k, mut v)| {
            v.sort_by(|a, b| a.0.unwrap_or(f64::MAX).total_cmp(&b.0.unwrap_or(f64::MAX)).then(a.1.cmp(&b.1)));
            (k, v.into_iter().map(|x| x.2).collect())
        })
        .collect()
}

/// Write a layout's words, morphs and glosses as the text's tokens and
/// morphs in one txn, and delete the tiers they came from (their segments
/// in the same txn, the tiers in a second, as tiers live in the project's
/// own scope). A sentence that already has words keeps them.
pub fn read_glossing(p: &mut Project, text: &str, layout: &Layout) -> Result<Read, String> {
    let mut read = Read::default();
    let mut segs: Vec<Seg> = Vec::new();
    let mut tier_of: Vec<String> = Vec::new();
    let mut wanted: Vec<&str> = [Some(layout.transcription.as_str()), Some(layout.words.as_str()), layout.morphs.as_deref(), layout.glosses.as_deref()].into_iter().flatten().collect();
    wanted.extend(layout.kept.iter().chain(&layout.dropped).map(|(id, _)| id.as_str()));
    for id in p.proj.ordered_children("text", text, "segment")? {
        let Some(tier) = p.proj.field_str(&id, "tier")? else { continue };
        if !wanted.contains(&tier.as_str()) {
            continue;
        }
        segs.push(Seg {
            content: p.proj.nfc(p.proj.field_str(&id, "content")?.unwrap_or_default().trim()),
            parent: p.proj.field_str(&id, "parent")?,
            start: p.proj.field_num(&id, "start")?,
            end: p.proj.field_num(&id, "end")?,
            id,
        });
        tier_of.push(tier);
    }
    let mut by_tier: HashMap<&str, Vec<&Seg>> = HashMap::new();
    for (s, t) in segs.iter().zip(&tier_of) {
        by_tier.entry(t.as_str()).or_default().push(s);
    }
    let by_id: HashMap<&str, &Seg> = segs.iter().map(|s| (s.id.as_str(), s)).collect();
    let sentences: Vec<&Seg> = by_tier.get(layout.transcription.as_str()).cloned().unwrap_or_default();
    let words = children_on(&by_tier, &layout.words, &sentences, &mut read.stray);
    let word_segs: Vec<&Seg> = by_tier.get(layout.words.as_str()).cloned().unwrap_or_default();
    let morphs = match &layout.morphs {
        Some(m) => children_on(&by_tier, m, &word_segs, &mut read.stray),
        None => HashMap::new(),
    };
    // The kept tiers' values by the word or morph they belong to.
    let mut extra: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for (id, name) in &layout.kept {
        for s in by_tier.get(id.as_str()).into_iter().flatten() {
            if let (Some(owner), false) = (&s.parent, s.content.is_empty()) {
                extra.entry(owner.clone()).or_default().push((format!("x.elan.{name}"), s.content.clone()));
            }
        }
    }
    let glosses: HashMap<String, String> = match &layout.glosses {
        Some(g) => by_tier
            .get(g.as_str())
            .into_iter()
            .flatten()
            .filter_map(|s| s.parent.clone().map(|o| (o, s.content.trim_matches(|c| matches!(c, '-' | '=')).trim().to_string())))
            .collect(),
        None => HashMap::new(),
    };
    let mut has_words = std::collections::HashSet::new();
    for sentence in &sentences {
        if !p.proj.members(&sentence.id)?.is_empty() {
            has_words.insert(sentence.id.clone());
        }
    }
    let mut t = p.intent("read-elan-glossing");
    for sentence in &sentences {
        let Some(ws) = words.get(&sentence.id) else { continue };
        if has_words.contains(&sentence.id) {
            read.kept += 1;
            continue;
        }
        read.sentences += 1;
        let mut wi = 0;
        for w in ws {
            let form = &by_id[w.as_str()].content;
            if form.is_empty() {
                continue;
            }
            let mut fields = vec![("segment", Value::str(&sentence.id)), ("pos", Value::str(&pos_seq(wi))), ("form", Value::str(form))];
            fields.extend(extra.get(w).into_iter().flatten().map(|(k, v)| (k.as_str(), Value::str(v))));
            let tok = t.create("token", "tok", text, fields);
            wi += 1;
            read.words += 1;
            // Its morphs, or with no morph tier the word itself glossed as one root.
            let pieces: Vec<(String, &'static str, Option<&String>, Option<&String>)> = match &layout.morphs {
                Some(_) => morphs
                    .get(w)
                    .into_iter()
                    .flatten()
                    .filter(|m| !by_id[m.as_str()].content.is_empty())
                    .map(|m| {
                        let (surface, kind) = morph_of(&by_id[m.as_str()].content);
                        (surface, kind, glosses.get(m), Some(m))
                    })
                    .collect(),
                None => glosses.get(w).map(|g| vec![(form.clone(), "root", Some(g), None)]).unwrap_or_default(),
            };
            for (mi, (surface, kind, gloss, source)) in pieces.into_iter().enumerate() {
                if surface.is_empty() {
                    continue;
                }
                let mut fields = vec![("token", Value::str(&tok)), ("pos", Value::str(&pos_seq(mi))), ("surface", Value::str(&surface)), ("type", Value::str(kind))];
                if let Some(g) = gloss.filter(|g| !g.is_empty()) {
                    fields.push(("gloss", Value::str(g)));
                    read.glossed += 1;
                }
                fields.extend(source.and_then(|m| extra.get(m)).into_iter().flatten().map(|(k, v)| (k.as_str(), Value::str(v))));
                t.create("morph", "mor", text, fields);
                read.morphs += 1;
            }
        }
    }
    // The tiers now live on as the glossing: their segments go, then the tiers.
    for (s, tier) in segs.iter().zip(&tier_of) {
        if tier != &layout.transcription {
            t.delete("segment", &s.id, text);
        }
    }
    t.commit()?;
    let mut t = p.intent("read-elan-glossing");
    for tier in &wanted[1..] {
        t.delete("tier", tier, SCOPE_CONFIG);
    }
    t.commit()?;
    Ok(read)
}
