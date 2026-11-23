//! Grambank (v1.0.2, CC BY 4.0; `datasets/grambank/`), embedded so the
//! Typology panel can show what Grambank says about a language's grammar and,
//! feature by feature, how the coded languages of its family (or, for a
//! family with too few, its macroarea) answer. A language Grambank has not
//! coded gets the same list as a questionnaire, with its relatives' answers
//! beside each question. Nothing is sent anywhere and nothing is written.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Serialize;

use crate::glottolog;

const FEATURES: &str = include_str!("../../../../datasets/grambank/features.tsv");
const VALUES: &str = include_str!("../../../../datasets/grambank/values.tsv");

pub const EDITION: &str = "Grambank 1.0 (CC BY 4.0)";

/// A family with fewer coded languages than this is compared with its macroarea.
const MIN_RELATIVES: usize = 3;

struct Feature {
    id: &'static str,
    question: &'static str,
    codes: Vec<(char, &'static str)>,
}

struct Data {
    features: Vec<Feature>,
    /// Glottocode → one byte per feature.
    values: HashMap<&'static str, &'static [u8]>,
}

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        let features = FEATURES
            .lines()
            .skip(1)
            .filter_map(|line| {
                let mut f = line.split('\t');
                let id = f.next()?;
                let question = f.next()?;
                let codes = f
                    .next()
                    .unwrap_or("")
                    .split('|')
                    .filter_map(|c| {
                        let (k, v) = c.split_once('=')?;
                        Some((k.chars().next()?, v))
                    })
                    .collect();
                Some(Feature { id, question, codes })
            })
            .collect();
        let values = VALUES
            .lines()
            .skip(1)
            .filter_map(|line| {
                let (code, v) = line.split_once('\t')?;
                Some((code, v.as_bytes()))
            })
            .collect();
        Data { features, values }
    })
}

/// How many of the compared languages give one answer.
#[derive(Debug, Serialize, PartialEq)]
pub struct Share {
    pub value: String,
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub id: String,
    pub question: String,
    /// The language's own value (`1`), `?` when Grambank's coders could not tell, None when not coded.
    pub value: Option<String>,
    /// What the value means (`present`).
    pub label: Option<String>,
    /// The compared languages' answers, commonest first; `?` and uncoded left out.
    pub others: Vec<Share>,
    /// Compared languages with an answer to this question.
    pub others_coded: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub glottocode: String,
    pub name: String,
    /// The languoid whose values are shown: the language itself, or for a dialect the language it belongs to.
    pub coded_as: Option<String>,
    pub coded_name: Option<String>,
    /// What the answers are compared with: a family's name or a macroarea, in words.
    pub compared_with: String,
    pub compared_count: usize,
    pub features: Vec<Answer>,
    pub edition: &'static str,
}

fn label_of(f: &Feature, v: char) -> Option<String> {
    f.codes.iter().find(|(k, _)| *k == v).map(|(_, l)| l.to_string())
}

/// What Grambank says about `glottocode`, with its relatives beside it.
pub fn profile(glottocode: &str) -> Result<Profile, String> {
    let me = glottolog::by_glottocode(glottocode).ok_or_else(|| format!("Glottolog has no languoid «{}».", glottocode.trim()))?;
    let d = data();
    // A dialect Grambank has not coded is read as the language it belongs to.
    let coded_as: Option<glottolog::Languoid> = if d.values.contains_key(me.glottocode) {
        Some(me.clone())
    } else if me.dialect && !me.language.is_empty() {
        glottolog::by_name(me.language, me.family).filter(|l| d.values.contains_key(l.glottocode))
    } else {
        None
    };
    let own: Option<&[u8]> = coded_as.as_ref().and_then(|l| d.values.get(l.glottocode).copied());
    let skip = coded_as.as_ref().map(|l| l.glottocode).unwrap_or(me.glottocode);
    let relatives = |pick: &dyn Fn(&glottolog::Languoid) -> bool| -> Vec<&'static [u8]> {
        d.values.iter().filter(|(code, _)| **code != skip).filter(|(code, _)| glottolog::by_glottocode(code).is_some_and(|l| pick(&l))).map(|(_, v)| *v).collect()
    };
    let in_family = !me.family.is_empty() && me.family != "isolate";
    let mut compared_with = String::new();
    let mut others: Vec<&'static [u8]> = Vec::new();
    if in_family {
        others = relatives(&|l| l.family == me.family);
        compared_with = format!("{} languages", me.family);
    }
    if others.len() < MIN_RELATIVES && !me.macroarea.is_empty() {
        others = relatives(&|l| l.macroarea == me.macroarea);
        compared_with = format!("languages of {}", me.macroarea);
    }
    let features = d
        .features
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let value = own.and_then(|v| v.get(i)).map(|b| *b as char).filter(|c| *c != '.');
            let mut counts: HashMap<char, usize> = HashMap::new();
            for o in &others {
                if let Some(c) = o.get(i).map(|b| *b as char).filter(|c| *c != '.' && *c != '?') {
                    *counts.entry(c).or_default() += 1;
                }
            }
            let others_coded = counts.values().sum();
            let mut shares: Vec<Share> = counts.into_iter().map(|(c, count)| Share { value: c.to_string(), label: label_of(f, c).unwrap_or_default(), count }).collect();
            shares.sort_by(|a, b| b.count.cmp(&a.count).then(a.value.cmp(&b.value)));
            Answer {
                id: f.id.to_string(),
                question: f.question.to_string(),
                label: value.and_then(|c| if c == '?' { Some("not known".to_string()) } else { label_of(f, c) }),
                value: value.map(|c| c.to_string()),
                others: shares,
                others_coded,
            }
        })
        .collect();
    Ok(Profile {
        glottocode: me.glottocode.to_string(),
        name: me.name.to_string(),
        coded_name: coded_as.as_ref().map(|l| l.name.to_string()),
        coded_as: coded_as.map(|l| l.glottocode.to_string()),
        compared_count: others.len(),
        compared_with,
        features,
        edition: EDITION,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_a_value_per_feature() {
        let d = data();
        assert_eq!(d.features.len(), 195);
        assert!(d.values.len() > 2400);
        for (code, v) in &d.values {
            assert_eq!(v.len(), d.features.len(), "{code}");
            assert!(v.iter().all(|b| b"0123?.".contains(b)), "{code}");
        }
        assert!(d.features.iter().all(|f| !f.codes.is_empty() && f.id.starts_with("GB")));
    }

    #[test]
    fn a_coded_language_shows_its_values_beside_its_family() {
        // Tagalog: Austronesian, coded in Grambank.
        let p = profile("taga1270").unwrap();
        assert_eq!(p.coded_as.as_deref(), Some("taga1270"));
        assert_eq!(p.compared_with, "Austronesian languages");
        assert!(p.compared_count > 100, "{}", p.compared_count);
        let articles = p.features.iter().find(|a| a.id == "GB020").unwrap();
        assert!(articles.value.is_some());
        assert!(articles.others_coded > 100);
        assert_eq!(articles.others.iter().map(|s| s.count).sum::<usize>(), articles.others_coded);
        assert!(articles.others.windows(2).all(|w| w[0].count >= w[1].count));
    }

    #[test]
    fn an_uncoded_language_still_gets_its_relatives_and_an_unknown_code_is_refused() {
        // Pick a Glottolog language Grambank has not coded, in a family it has coded well.
        let d = data();
        let uncoded = glottolog::all().into_iter().find(|l| !l.dialect && l.family == "Austronesian" && !d.values.contains_key(l.glottocode)).unwrap();
        let p = profile(uncoded.glottocode).unwrap();
        assert_eq!(p.coded_as, None);
        assert!(p.features.iter().all(|a| a.value.is_none()));
        assert!(p.features.iter().any(|a| a.others_coded > 50));
        assert!(profile("zzzz9999").is_err());
    }
}
