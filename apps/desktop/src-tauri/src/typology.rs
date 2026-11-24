//! Grambank (v1.0.2) and WALS (v2020.6), both CC BY 4.0 (`datasets/grambank/`,
//! `datasets/wals/`, written in one shape), embedded so the Typology panel can
//! show what either says about a language's grammar and,
//! feature by feature, how the coded languages of its family (or, for a
//! family with too few, its macroarea) answer. A language Grambank has not
//! coded gets the same list as a questionnaire, with its relatives' answers
//! beside each question. Nothing is sent anywhere. The project's own answers
//! for its language are project settings, one per feature
//! (`typology:<dataset>:<feature>` for the value, `typology-note:…` for a
//! note), so they sync and two people answering different questions never
//! conflict.
//! A value is one character per feature: the code, `?` for not known, `.`
//! for not coded.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Serialize;

use crate::glottolog;
use crate::session::{self, Session};

/// A dataset the panel can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Grambank,
    Wals,
}

impl Source {
    fn files(self) -> (&'static str, &'static str) {
        match self {
            Source::Grambank => (include_str!("../../../../datasets/grambank/features.tsv"), include_str!("../../../../datasets/grambank/values.tsv")),
            Source::Wals => (include_str!("../../../../datasets/wals/features.tsv"), include_str!("../../../../datasets/wals/values.tsv")),
        }
    }

    fn key(self) -> &'static str {
        match self {
            Source::Grambank => "grambank",
            Source::Wals => "wals",
        }
    }

    fn edition(self) -> &'static str {
        match self {
            Source::Grambank => "Grambank 1.0 (CC BY 4.0): Skirgård et al. 2023, Science Advances",
            Source::Wals => "WALS Online v2020 (CC BY 4.0): Dryer & Haspelmath (eds.) 2013",
        }
    }

    fn page(self, glottocode: &str) -> String {
        match (self, data(self).ids.get(glottocode)) {
            (Source::Grambank, _) => format!("https://grambank.clld.org/languages/{glottocode}"),
            (Source::Wals, Some(id)) => format!("https://wals.info/languoid/lect/wals_code_{id}"),
            (Source::Wals, None) => "https://wals.info".to_string(),
        }
    }
}

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
    /// Glottocode → the dataset's own ID for it, when it has one (WALS codes).
    ids: HashMap<&'static str, &'static str>,
}

fn data(source: Source) -> &'static Data {
    static GRAMBANK: OnceLock<Data> = OnceLock::new();
    static WALS: OnceLock<Data> = OnceLock::new();
    let cell = match source {
        Source::Grambank => &GRAMBANK,
        Source::Wals => &WALS,
    };
    let (features_tsv, values_tsv) = source.files();
    cell.get_or_init(|| {
        let features = features_tsv
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
        let mut values = HashMap::new();
        let mut ids = HashMap::new();
        for line in values_tsv.lines().skip(1) {
            let mut f = line.split('\t');
            let (Some(code), Some(v)) = (f.next(), f.next()) else { continue };
            values.insert(code, v.as_bytes());
            if let Some(id) = f.next().filter(|id| !id.is_empty()) {
                ids.insert(code, id);
            }
        }
        Data { features, values, ids }
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
    /// Every answer the question allows, in the dataset's order.
    pub codes: Vec<Code>,
}

/// One answer a question allows.
#[derive(Debug, Serialize, PartialEq)]
pub struct Code {
    pub value: String,
    pub label: String,
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
    /// The dataset and how to cite it.
    pub edition: &'static str,
    /// Where the sources behind the values are read.
    pub page: String,
}

/// The coded languages `me` is compared with, by Glottocode in order, and
/// what they are in words: its family's, or its macroarea's when the family
/// has fewer than `MIN_RELATIVES` (`skip` is the languoid whose values are
/// shown, left out).
fn compared(me: &glottolog::Languoid, d: &Data, skip: &str) -> (Vec<&'static str>, String) {
    let relatives = |pick: &dyn Fn(&glottolog::Languoid) -> bool| -> Vec<&'static str> {
        let mut v: Vec<&'static str> = d.values.keys().copied().filter(|code| *code != skip).filter(|code| glottolog::by_glottocode(code).is_some_and(|l| pick(&l))).collect();
        v.sort_unstable();
        v
    };
    let in_family = !me.family.is_empty() && me.family != "isolate";
    let mut compared_with = String::new();
    let mut others: Vec<&'static str> = Vec::new();
    if in_family {
        others = relatives(&|l| l.family == me.family);
        compared_with = format!("{} languages", me.family);
    }
    if others.len() < MIN_RELATIVES && !me.macroarea.is_empty() {
        others = relatives(&|l| l.macroarea == me.macroarea);
        compared_with = format!("languages of {}", me.macroarea);
    }
    (others, compared_with)
}

/// The world's land as an SVG path in longitude/latitude (x = lon + 180, y = 90 − lat).
pub const LAND: &str = include_str!("../../../../datasets/naturalearth/land-110m.path");

/// One language on a feature's map.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapPoint {
    pub glottocode: String,
    pub name: String,
    pub lat: f32,
    pub lon: f32,
    /// Its value for the feature (`?` not known), or None when not coded.
    pub value: Option<String>,
    /// The language the map is about (else one it is compared with).
    pub me: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureMap {
    pub feature: String,
    pub question: String,
    pub codes: Vec<Code>,
    pub compared_with: String,
    /// The compared languages with an answer and coordinates, then the language itself.
    pub points: Vec<MapPoint>,
    /// Compared languages with an answer that Glottolog gives no place for.
    pub unplaced: usize,
    pub land: &'static str,
    pub edition: &'static str,
}

/// Where the languages `glottocode` is compared with answer one feature, on a map.
pub fn map(glottocode: &str, source: Source, feature: &str) -> Result<FeatureMap, String> {
    let me = glottolog::by_glottocode(glottocode).ok_or_else(|| format!("Glottolog has no languoid «{}».", glottocode.trim()))?;
    let d = data(source);
    let index = d.features.iter().position(|f| f.id == feature).ok_or_else(|| format!("{} has no feature «{feature}».", source.edition()))?;
    let f = &d.features[index];
    let coded_as = if d.values.contains_key(me.glottocode) {
        Some(me.clone())
    } else if me.dialect && !me.language.is_empty() {
        glottolog::by_name(me.language, me.family).filter(|l| d.values.contains_key(l.glottocode))
    } else {
        None
    };
    let skip = coded_as.as_ref().map(|l| l.glottocode).unwrap_or(me.glottocode);
    let (group, compared_with) = compared(&me, d, skip);
    let value_of = |code: &str| d.values.get(code).and_then(|v| v.get(index)).map(|b| *b as char).filter(|c| *c != '.').map(|c| c.to_string());
    let mut points = Vec::new();
    let mut unplaced = 0;
    for code in group {
        let Some(value) = value_of(code) else { continue };
        match glottolog::by_glottocode(code) {
            Some(l) if l.lat.is_some() && l.lon.is_some() => points.push(MapPoint { glottocode: code.to_string(), name: l.name.to_string(), lat: l.lat.unwrap_or(0.0), lon: l.lon.unwrap_or(0.0), value: Some(value), me: false }),
            _ => unplaced += 1,
        }
    }
    // The language itself last, so it is drawn on top; placed by itself, else by the language it was read as.
    let place = [Some(me.clone()), coded_as.clone()].into_iter().flatten().find(|l| l.lat.is_some() && l.lon.is_some());
    if let Some(at) = place {
        points.push(MapPoint { glottocode: me.glottocode.to_string(), name: me.name.to_string(), lat: at.lat.unwrap_or(0.0), lon: at.lon.unwrap_or(0.0), value: value_of(skip), me: true });
    }
    Ok(FeatureMap {
        feature: f.id.to_string(),
        question: f.question.to_string(),
        codes: f.codes.iter().map(|(k, l)| Code { value: k.to_string(), label: l.to_string() }).collect(),
        compared_with,
        points,
        unplaced,
        land: LAND,
        edition: source.edition(),
    })
}

fn label_of(f: &Feature, v: char) -> Option<String> {
    f.codes.iter().find(|(k, _)| *k == v).map(|(_, l)| l.to_string())
}

/// What `source` says about `glottocode`, with its relatives beside it.
pub fn profile(glottocode: &str, source: Source) -> Result<Profile, String> {
    let me = glottolog::by_glottocode(glottocode).ok_or_else(|| format!("Glottolog has no languoid «{}».", glottocode.trim()))?;
    let d = data(source);
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
    let (group, compared_with) = compared(&me, d, skip);
    let others: Vec<&'static [u8]> = group.iter().filter_map(|code| d.values.get(code).copied()).collect();
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
                codes: f.codes.iter().map(|(k, l)| Code { value: k.to_string(), label: l.to_string() }).collect(),
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
        edition: source.edition(),
        page: source.page(skip),
    })
}

/// The project's own answer to one feature.
#[derive(Debug, Serialize, PartialEq)]
pub struct Ours {
    pub id: String,
    /// A code the question allows, or `?` for asked but not known.
    pub value: String,
    pub note: String,
}

/// The longest note kept (in characters).
const MAX_NOTE: usize = 1000;

fn value_key(source: Source, id: &str) -> String {
    format!("typology:{}:{id}", source.key())
}

fn note_key(source: Source, id: &str) -> String {
    format!("typology-note:{}:{id}", source.key())
}

/// The project's answers for `source`, in the dataset's feature order; a
/// feature with neither a value nor a note is left out.
pub fn answers(session: &mut Session, source: Source) -> Result<Vec<Ours>, String> {
    let p = session::project(session)?;
    let mut out = Vec::new();
    for f in &data(source).features {
        let value = p.setting(&value_key(source, f.id))?.unwrap_or_default();
        let note = p.setting(&note_key(source, f.id))?.unwrap_or_default();
        if !value.is_empty() || !note.is_empty() {
            out.push(Ours { id: f.id.to_string(), value, note });
        }
    }
    Ok(out)
}

/// Write the project's answer to feature `id`: `value` is one of its codes,
/// `?`, or empty to clear; the note is trimmed, its spaces collapsed, NFC.
/// A part that already says that is not written.
pub fn set_answer(session: &mut Session, source: Source, id: &str, value: &str, note: &str) -> Result<Ours, String> {
    let f = data(source).features.iter().find(|f| f.id == id).ok_or_else(|| format!("{} has no feature «{id}».", source.edition()))?;
    let value = value.trim();
    if !(value.is_empty() || value == "?" || f.codes.iter().any(|(k, _)| k.to_string() == value)) {
        return Err(format!("«{value}» is not an answer to {id}."));
    }
    let p = session::project(session)?;
    let note = p.proj.nfc(&note.split_whitespace().collect::<Vec<_>>().join(" "));
    if note.chars().count() > MAX_NOTE {
        return Err(format!("A note is at most {MAX_NOTE} characters."));
    }
    for (k, v) in [(value_key(source, id), value), (note_key(source, id), note.as_str())] {
        if p.setting(&k)?.unwrap_or_default() != v {
            p.set_setting(&k, v)?;
        }
    }
    Ok(Ours { id: id.to_string(), value: value.to_string(), note })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_projects_answers_are_kept_checked_and_cleared() {
        let dir = std::env::temp_dir().join(format!("wugs-typology-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        assert_eq!(answers(&mut session, Source::Grambank).unwrap(), vec![]);
        set_answer(&mut session, Source::Grambank, "GB020", "1", "  the  word\n«ang» ").unwrap();
        set_answer(&mut session, Source::Wals, "81A", "2", "").unwrap();
        let ours = answers(&mut session, Source::Grambank).unwrap();
        assert_eq!(ours, vec![Ours { id: "GB020".into(), value: "1".into(), note: "the word «ang»".into() }]);
        assert_eq!(answers(&mut session, Source::Wals).unwrap()[0].value, "2");
        let before = session::project(&mut session).unwrap().proj.max_hlc().unwrap();
        set_answer(&mut session, Source::Grambank, "GB020", "1", "the word «ang»").unwrap();
        assert_eq!(session::project(&mut session).unwrap().proj.max_hlc().unwrap(), before, "the same answer writes nothing");
        assert!(set_answer(&mut session, Source::Grambank, "GB020", "7", "").is_err());
        assert!(set_answer(&mut session, Source::Grambank, "GB999", "1", "").is_err());
        set_answer(&mut session, Source::Grambank, "GB020", "?", "").unwrap();
        assert_eq!(answers(&mut session, Source::Grambank).unwrap()[0].value, "?");
        set_answer(&mut session, Source::Grambank, "GB020", "", "").unwrap();
        assert_eq!(answers(&mut session, Source::Grambank).unwrap(), vec![]);
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_language_has_a_value_per_feature_and_every_value_a_meaning() {
        for (source, features, languages) in [(Source::Grambank, 195, 2400), (Source::Wals, 192, 2400)] {
            let d = data(source);
            assert_eq!(d.features.len(), features);
            assert!(d.values.len() > languages, "{source:?} {}", d.values.len());
            for (code, v) in &d.values {
                assert_eq!(v.len(), d.features.len(), "{code}");
                for (i, b) in v.iter().enumerate() {
                    let c = *b as char;
                    assert!(c == '.' || c == '?' || label_of(&d.features[i], c).is_some(), "{source:?} {code} {} {c}", d.features[i].id);
                }
            }
            assert!(d.features.iter().all(|f| !f.codes.is_empty()));
        }
    }

    #[test]
    fn wals_reads_like_grambank() {
        // English in WALS: 81A Order of Subject, Object and Verb is SVO.
        let p = profile("stan1293", Source::Wals).unwrap();
        let svo = p.features.iter().find(|a| a.id == "81A").unwrap();
        assert_eq!(svo.label.as_deref(), Some("SVO"));
        assert_eq!(p.page, "https://wals.info/languoid/lect/wals_code_eng");
        assert!(p.compared_with.starts_with("Indo-European"), "{}", p.compared_with);
    }

    #[test]
    fn a_coded_language_shows_its_values_beside_its_family() {
        // Tagalog: Austronesian, coded in Grambank.
        let p = profile("taga1270", Source::Grambank).unwrap();
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
        let d = data(Source::Grambank);
        let uncoded = glottolog::all().into_iter().find(|l| !l.dialect && l.family == "Austronesian" && !d.values.contains_key(l.glottocode)).unwrap();
        let p = profile(uncoded.glottocode, Source::Grambank).unwrap();
        assert_eq!(p.coded_as, None);
        assert!(p.features.iter().all(|a| a.value.is_none()));
        assert!(p.features.iter().any(|a| a.others_coded > 50));
        assert!(profile("zzzz9999", Source::Wals).is_err());
    }

    #[test]
    fn a_feature_map_places_the_relatives_and_the_language() {
        let d = data(Source::Grambank);
        // A coded Austronesian language Glottolog places.
        let me = glottolog::all().into_iter().find(|l| !l.dialect && l.family == "Austronesian" && l.lat.is_some() && d.values.contains_key(l.glottocode)).unwrap();
        let feature = d.features[0].id;
        let m = map(me.glottocode, Source::Grambank, feature).unwrap();
        assert_eq!(m.feature, feature);
        assert!(m.compared_with.starts_with("Austronesian"));
        let last = m.points.last().unwrap();
        assert!(last.me && last.glottocode == me.glottocode, "the language itself is drawn last");
        assert!(m.points.len() > 20);
        assert!(m.points.iter().all(|p| (-90.0..=90.0).contains(&p.lat) && (-180.0..=180.0).contains(&p.lon)));
        // Every compared point has an answer the feature allows (or `?`).
        assert!(m.points.iter().filter(|p| !p.me).all(|p| p.value.as_deref().is_some_and(|v| v == "?" || m.codes.iter().any(|c| c.value == v))));
        // The map's compared set is the profile's.
        let profile = profile(me.glottocode, Source::Grambank).unwrap();
        assert_eq!(profile.compared_with, m.compared_with);
        assert!(m.points.len() - 1 + m.unplaced <= profile.compared_count);
        assert!(m.land.starts_with('M') && m.land.len() > 10_000);
        assert!(map(me.glottocode, Source::Grambank, "GB_nope").is_err());
        assert_eq!(glottolog::by_glottocode("stan1293").unwrap().lat, Some(53.0));
    }
}
