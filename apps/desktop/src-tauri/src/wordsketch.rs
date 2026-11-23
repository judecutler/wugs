//! Word sketch: a lemma's grammatical company, read from the dependency
//! trees of the parsed sentences (Sketch Engine's word sketch over Universal
//! Dependencies). For each relation the lemma takes dependents in (its
//! objects, subjects, modifiers…) and each relation it attaches to a head in
//! (the verbs it is the object of…), the lemmas found there, with how often
//! and logDice, Sketch Engine's association score. Only sentences with an
//! `analysis` (CoNLL-U, TEI, a tagger or corrections) count; punctuation
//! relations are left out.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;
use wugs_model::fold_key;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{project, Session};

/// Collocates listed per relation.
pub const PER_GROUP: usize = 25;

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SketchItem {
    /// The lemma as met most often.
    pub lemma: String,
    pub count: usize,
    pub log_dice: f64,
    /// Times the collocate stood after the node (the rest it stood before).
    pub after: usize,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SketchGroup {
    /// The UD relation (`obj`, `nmod:poss`).
    pub relation: String,
    /// `dependent`: words that depend on the lemma by this relation;
    /// `head`: words the lemma depends on by it.
    pub direction: String,
    pub total: usize,
    pub items: Vec<SketchItem>,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct WordSketch {
    /// The lemma as met most often (empty when not found).
    pub lemma: String,
    /// Its uses in parsed sentences.
    pub count: usize,
    /// Parsed sentences read.
    pub parsed: usize,
    /// Most filled first.
    pub groups: Vec<SketchGroup>,
}

/// A word's lemma, else its form: what a collocate is counted under.
fn lemma_of(row: &wugs_conllu::tree::Row) -> &str {
    if row.lemma.is_empty() || row.lemma == "_" {
        &row.form
    } else {
        &row.lemma
    }
}

/// Sketch Engine's logDice: 14 + log2(2·f(xy) / (f(x) + f(y))).
pub fn log_dice(together: usize, node: usize, other: usize) -> f64 {
    if together == 0 || node + other == 0 {
        return 0.0;
    }
    14.0 + (2.0 * together as f64 / (node + other) as f64).log2()
}

#[derive(Default)]
struct Pair {
    count: usize,
    after: usize,
}

pub fn word_sketch(session: &mut Session, lemma: &str) -> Result<WordSketch, String> {
    let p = project(session)?;
    let node = fold_key(&p.proj.nfc(lemma.trim()));
    let mut out = WordSketch::default();
    if node.is_empty() {
        return Ok(out);
    }
    let mut freq: HashMap<String, usize> = HashMap::new();
    let mut spellings: HashMap<String, BTreeMap<String, usize>> = HashMap::new();
    let mut pairs: BTreeMap<(String, String, String), Pair> = BTreeMap::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        if p.proj.field(&seg.id, "analysis")?.is_none() {
            return Ok(());
        }
        out.parsed += 1;
        let rows = wugs_conllu::tree::rows(p, &seg.id)?.rows;
        let keys: Vec<String> = rows.iter().map(|r| fold_key(&p.proj.nfc(lemma_of(r)))).collect();
        for (r, k) in rows.iter().zip(&keys) {
            if r.upos == "PUNCT" || k.is_empty() {
                continue;
            }
            *freq.entry(k.clone()).or_default() += 1;
            *spellings.entry(k.clone()).or_default().entry(p.proj.nfc(lemma_of(r))).or_default() += 1;
        }
        let mut add = |rel: &str, dir: &str, other: usize, me: usize| {
            let rel = rel.trim();
            if rel.is_empty() || rel == "_" || rel == "punct" || rel == "root" || keys[other].is_empty() || rows[other].upos == "PUNCT" {
                return;
            }
            let e = pairs.entry((rel.to_string(), dir.to_string(), keys[other].clone())).or_default();
            e.count += 1;
            if other > me {
                e.after += 1;
            }
        };
        for (i, k) in keys.iter().enumerate() {
            if *k != node {
                continue;
            }
            // Its dependents.
            for (j, r) in rows.iter().enumerate() {
                if r.head == Some(i + 1) {
                    add(&r.deprel, "dependent", j, i);
                }
            }
            // Its head.
            if let Some(h) = rows[i].head.filter(|h| *h >= 1 && *h <= rows.len()) {
                add(&rows[i].deprel, "head", h - 1, i);
            }
        }
        Ok(())
    })?;
    out.count = freq.get(&node).copied().unwrap_or(0);
    let best = |k: &str| spellings.get(k).and_then(|m| m.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(s, _)| s.clone())).unwrap_or_else(|| k.to_string());
    out.lemma = if out.count > 0 { best(&node) } else { String::new() };
    let mut groups: BTreeMap<(String, String), SketchGroup> = BTreeMap::new();
    for ((rel, dir, other), pair) in pairs {
        let g = groups.entry((rel.clone(), dir.clone())).or_insert_with(|| SketchGroup { relation: rel, direction: dir, total: 0, items: Vec::new() });
        g.total += pair.count;
        let f = freq.get(&other).copied().unwrap_or(0);
        g.items.push(SketchItem { lemma: best(&other), count: pair.count, log_dice: log_dice(pair.count, out.count, f), after: pair.after });
    }
    let mut groups: Vec<SketchGroup> = groups.into_values().collect();
    for g in &mut groups {
        g.items.sort_by(|a, b| b.log_dice.total_cmp(&a.log_dice).then(b.count.cmp(&a.count)).then(a.lemma.cmp(&b.lemma)));
        g.items.truncate(PER_GROUP);
    }
    groups.sort_by(|a, b| b.total.cmp(&a.total).then(a.direction.cmp(&b.direction)).then(a.relation.cmp(&b.relation)));
    out.groups = groups;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_dice_is_14_for_words_always_together() {
        assert_eq!(log_dice(5, 5, 5), 14.0);
        assert_eq!(log_dice(1, 4, 4), 12.0);
        assert_eq!(log_dice(0, 4, 4), 0.0);
    }

    #[test]
    fn a_lemma_s_objects_subjects_and_heads() {
        let dir = std::env::temp_dir().join(format!("wugs-sketch-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let p = project(&mut s).unwrap();
        let tier = crate::session::utterance_tier(p).unwrap();
        let ud = "\
1\tDogs\tdog\tNOUN\t_\t_\t2\tnsubj\t_\t_
2\teat\teat\tVERB\t_\t_\t0\troot\t_\t_
3\tbones\tbone\tNOUN\t_\t_\t2\tobj\t_\tSpaceAfter=No
4\t.\t.\tPUNCT\t_\t_\t2\tpunct\t_\t_

1\tCats\tcat\tNOUN\t_\t_\t2\tnsubj\t_\t_
2\tate\teat\tVERB\t_\t_\t0\troot\t_\t_
3\tfish\tfish\tNOUN\t_\t_\t2\tobj\t_\t_

1\tThe\tthe\tDET\t_\t_\t2\tdet\t_\t_
2\tdog\tdog\tNOUN\t_\t_\t3\tnsubj\t_\t_
3\tsleeps\tsleep\tVERB\t_\t_\t0\troot\t_\t_

";
        wugs_conllu::import_conllu(p, ud, "animals", &tier).unwrap();
        let w = word_sketch(&mut s, "EAT").unwrap();
        assert_eq!((w.lemma.as_str(), w.count, w.parsed), ("eat", 2, 3));
        let groups: Vec<_> = w.groups.iter().map(|g| (g.relation.as_str(), g.direction.as_str(), g.items.iter().map(|i| (i.lemma.as_str(), i.count)).collect::<Vec<_>>())).collect();
        assert_eq!(groups, [("nsubj", "dependent", vec![("cat", 1), ("dog", 1)]), ("obj", "dependent", vec![("bone", 1), ("fish", 1)])], "punct and root left out");
        let cat = &w.groups[0].items[0];
        assert_eq!(cat.after, 0, "the subject stands before the verb");
        assert!((cat.log_dice - log_dice(1, 2, 1)).abs() < 1e-9);
        let dog = word_sketch(&mut s, "dog").unwrap();
        let heads: Vec<_> = dog.groups.iter().map(|g| (g.relation.as_str(), g.direction.as_str(), g.items.iter().map(|i| i.lemma.as_str()).collect::<Vec<_>>())).collect();
        assert_eq!(heads, [("nsubj", "head", vec!["sleep", "eat"]), ("det", "dependent", vec!["the"])]);
        assert!(word_sketch(&mut s, "giraffe").unwrap().lemma.is_empty());
        // The query the panel opens for a collocate finds its lines.
        let c = crate::cqp::run(&mut s, r#"[lemma="EAT" %c] []{0,10} [lemma="bone" %c]"#, None).unwrap();
        assert_eq!(c.total, 1);
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
