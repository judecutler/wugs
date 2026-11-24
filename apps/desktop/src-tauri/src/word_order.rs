//! Word order read from the dependency trees of the parsed sentences, as
//! WALS describes it (chapters 81–90 and 143): for each construction, how
//! often the dependent comes before its head and how often after, with an
//! example sentence of each, so a typologist can see the dominant order and
//! the project can answer WALS from its own data. Only sentences with an
//! `analysis` (CoNLL-U, TEI, a tagger or corrections) count. Subjects and
//! objects are counted when nominal (a noun or proper noun), as WALS counts
//! them; pronouns often order differently.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{project, Session};
use wugs_conllu::tree::Row;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Example {
    pub text_id: String,
    pub tier_id: String,
    pub segment_id: String,
    pub sentence: String,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    /// WALS's name for it (`SVO`, `Prepositions`).
    pub label: String,
    /// Its WALS code for the feature.
    pub code: String,
    pub count: usize,
    pub example: Option<Example>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pattern {
    /// The WALS feature (`83A`).
    pub id: String,
    pub title: String,
    pub orders: Vec<Order>,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct WordOrder {
    /// Parsed sentences read.
    pub parsed: usize,
    pub patterns: Vec<Pattern>,
}

/// Each feature: its title and its orders (label and WALS code), in WALS's order.
const FEATURES: &[(&str, &str, &[(&str, &str)])] = &[
    ("81A", "Order of subject, object and verb", &[("SOV", "1"), ("SVO", "2"), ("VSO", "3"), ("VOS", "4"), ("OVS", "5"), ("OSV", "6")]),
    ("82A", "Order of subject and verb", &[("SV", "1"), ("VS", "2")]),
    ("83A", "Order of object and verb", &[("OV", "1"), ("VO", "2")]),
    ("85A", "Order of adposition and noun phrase", &[("Postpositions", "1"), ("Prepositions", "2")]),
    ("86A", "Order of genitive and noun", &[("Genitive-Noun", "1"), ("Noun-Genitive", "2")]),
    ("87A", "Order of adjective and noun", &[("Adjective-Noun", "1"), ("Noun-Adjective", "2")]),
    ("88A", "Order of demonstrative and noun", &[("Demonstrative-Noun", "1"), ("Noun-Demonstrative", "2")]),
    ("89A", "Order of numeral and noun", &[("Numeral-Noun", "1"), ("Noun-Numeral", "2")]),
    ("90A", "Order of relative clause and noun", &[("Noun-Relative clause", "1"), ("Relative clause-Noun", "2")]),
    ("143A", "Order of negative morpheme and verb", &[("NegV", "1"), ("VNeg", "2")]),
];

fn has_feat(r: &Row, feat: &str) -> bool {
    r.feats.split('|').any(|f| f == feat)
}

const NOMINAL: &[&str] = &["NOUN", "PROPN"];
const NOUNISH: &[&str] = &["NOUN", "PROPN", "PRON"];

/// The order a dependent in a construction shows, as an index into its
/// feature's orders, with the feature; None when it is not one WALS counts.
fn classify(rows: &[Row], dep: usize) -> Option<(&'static str, usize)> {
    let r = &rows[dep];
    let head = r.head.filter(|h| *h >= 1 && *h <= rows.len())? - 1;
    let h = &rows[head];
    let before = dep < head;
    let pick = |id: &'static str, dep_first: usize, head_first: usize| Some((id, if before { dep_first } else { head_first }));
    let rel = r.deprel.trim();
    let base = rel.split(':').next().unwrap_or(rel);
    match base {
        "obj" if h.upos == "VERB" && NOMINAL.contains(&r.upos.as_str()) => pick("83A", 0, 1),
        "nsubj" if rel == "nsubj" && h.upos == "VERB" && NOMINAL.contains(&r.upos.as_str()) => pick("82A", 0, 1),
        "case" if r.upos == "ADP" && NOUNISH.contains(&h.upos.as_str()) => pick("85A", 1, 0),
        "nmod" if (rel == "nmod:poss" || has_feat(r, "Case=Gen")) && NOMINAL.contains(&r.upos.as_str()) && NOMINAL.contains(&h.upos.as_str()) => pick("86A", 0, 1),
        "amod" if r.upos == "ADJ" && h.upos == "NOUN" => pick("87A", 0, 1),
        "det" if has_feat(r, "PronType=Dem") && NOMINAL.contains(&h.upos.as_str()) => pick("88A", 0, 1),
        "nummod" if r.upos == "NUM" && h.upos == "NOUN" => pick("89A", 0, 1),
        "acl" if rel == "acl:relcl" && NOUNISH.contains(&h.upos.as_str()) => pick("90A", 1, 0),
        "advmod" | "aux" | "neg" | "discourse" if has_feat(r, "Polarity=Neg") && h.upos == "VERB" => pick("143A", 0, 1),
        _ => None,
    }
}

/// For a verb with a nominal subject and a nominal object, which of SOV … OSV.
fn clause(rows: &[Row], verb: usize) -> Option<usize> {
    let of = |want: &str| rows.iter().position(|r| r.head == Some(verb + 1) && r.deprel.trim() == want && NOMINAL.contains(&r.upos.as_str()));
    let (s, o) = (of("nsubj")?, of("obj")?);
    let mut three = [('S', s), ('O', o), ('V', verb)];
    three.sort_by_key(|x| x.1);
    let order: String = three.iter().map(|x| x.0).collect();
    ["SOV", "SVO", "VSO", "VOS", "OVS", "OSV"].iter().position(|x| *x == order)
}

pub fn word_order(session: &mut Session) -> Result<WordOrder, String> {
    let p = project(session)?;
    let mut out = WordOrder::default();
    let mut counts: BTreeMap<(&'static str, usize), (usize, Option<Example>)> = BTreeMap::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        if p.proj.field(&seg.id, "analysis")?.is_none() {
            return Ok(());
        }
        out.parsed += 1;
        let rows = wugs_conllu::tree::rows(p, &seg.id)?.rows;
        let mut add = |key: (&'static str, usize)| {
            let e = counts.entry(key).or_insert((0, None));
            e.0 += 1;
            if e.1.is_none() {
                let sentence = rows.iter().map(|r| r.form.as_str()).collect::<Vec<_>>().join(" ");
                e.1 = Some(Example { text_id: seg.text.clone(), tier_id: seg.tier.clone(), segment_id: seg.id.clone(), sentence });
            }
        };
        for i in 0..rows.len() {
            if let Some(key) = classify(&rows, i) {
                add(key);
            }
            if rows[i].upos == "VERB" {
                if let Some(k) = clause(&rows, i) {
                    add(("81A", k));
                }
            }
        }
        Ok(())
    })?;
    for (id, title, orders) in FEATURES {
        let orders: Vec<Order> = orders
            .iter()
            .enumerate()
            .map(|(k, (label, code))| {
                let (count, example) = counts.remove(&(*id, k)).unwrap_or((0, None));
                Order { label: label.to_string(), code: code.to_string(), count, example }
            })
            .collect();
        out.patterns.push(Pattern { id: id.to_string(), title: title.to_string(), orders });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_are_counted_from_the_trees() {
        let dir = std::env::temp_dir().join(format!("wugs-order-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let p = project(&mut s).unwrap();
        let tier = crate::session::utterance_tier(p).unwrap();
        let ud = "\
1\tDogs\tdog\tNOUN\t_\t_\t2\tnsubj\t_\t_
2\teat\teat\tVERB\t_\t_\t0\troot\t_\t_
3\tbig\tbig\tADJ\t_\t_\t4\tamod\t_\t_
4\tbones\tbone\tNOUN\t_\t_\t2\tobj\t_\t_
5\tin\tin\tADP\t_\t_\t7\tcase\t_\t_
6\tthree\tthree\tNUM\t_\t_\t7\tnummod\t_\t_
7\tyards\tyard\tNOUN\t_\t_\t2\tobl\t_\t_

1\tShe\tshe\tPRON\t_\t_\t3\tnsubj\t_\t_
2\tnot\tnot\tPART\t_\tPolarity=Neg\t3\tadvmod\t_\t_
3\tsee\tsee\tVERB\t_\t_\t0\troot\t_\t_
4\tthis\tthis\tDET\t_\tPronType=Dem\t5\tdet\t_\t_
5\tfox\tfox\tNOUN\t_\t_\t3\tobj\t_\t_
6\tfox\tfox\tNOUN\t_\t_\t3\tobj\t_\t_

";
        wugs_conllu::import_conllu(p, ud, "orders", &tier).unwrap();
        let w = word_order(&mut s).unwrap();
        assert_eq!(w.parsed, 2);
        let count = |id: &str, label: &str| w.patterns.iter().find(|x| x.id == id).unwrap().orders.iter().find(|o| o.label == label).unwrap().count;
        assert_eq!(count("81A", "SVO"), 1, "a pronoun subject is not counted");
        assert_eq!(count("82A", "SV"), 1);
        assert_eq!(count("83A", "VO"), 3);
        assert_eq!(count("85A", "Prepositions"), 1);
        assert_eq!(count("87A", "Adjective-Noun"), 1);
        assert_eq!(count("88A", "Demonstrative-Noun"), 1);
        assert_eq!(count("89A", "Numeral-Noun"), 1);
        assert_eq!(count("143A", "NegV"), 1);
        assert_eq!(count("86A", "Noun-Genitive"), 0);
        let ex = w.patterns.iter().find(|x| x.id == "143A").unwrap().orders[0].example.clone().unwrap();
        assert_eq!(ex.sentence, "She not see this fox fox");
        assert_eq!(w.patterns.len(), FEATURES.len());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
