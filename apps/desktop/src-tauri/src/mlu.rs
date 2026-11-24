//! Speakers: how long each speaker's utterances are, the measure child
//! language research starts from (Brown 1973; CLAN's MLU). A speaker is a
//! transcription track, as CHAT files come in a track per speaker.
//!
//! - **MLU in words:** running words per utterance, the way the corpus
//!   tools count words; CHAT's unintelligible `xxx`, `yyy` and `www` are
//!   not words.
//! - **MLU in morphemes:** for an utterance whose every word is glossed, its
//!   morphs; else, for one that came from CHAT with a `%mor` tier, the
//!   morphemes CLAN counts there (a stem, each `-` suffix, each `#` prefix,
//!   each clitic; fused `&` forms and compounds count once). Utterances with
//!   neither are left out of this one measure and counted.
//! - **MTLD:** lexical diversity that does not shrink as a speaker says
//!   more, as type/token ratios do (McCarthy and Jarvis 2010): the mean
//!   run of words over which the type/token ratio stays above 0.72, read
//!   forwards and backwards. Only from 100 words up.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{project, Session};

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerRow {
    pub tier: String,
    pub name: String,
    pub texts: usize,
    /// Utterances with at least one word.
    pub utterances: usize,
    pub words: usize,
    /// Different words (folded).
    pub types: usize,
    /// The most words in one utterance.
    pub longest: usize,
    /// Utterances whose morphemes are known, and how many morphemes they hold.
    pub analysed: usize,
    pub morphemes: usize,
    /// MTLD over the speaker's running words, from `MTLD_MIN` words up.
    pub mtld: Option<f64>,
    /// The track's speaker in the People panel, with their sex and age
    /// details as written there (empty when none is set).
    pub person: String,
    pub sex: String,
    pub age: String,
    /// The person's `born` detail, so an age can be worked out from a text's date.
    pub born: String,
    /// The same figures text by text, for lengths over time (a child's MLU by age).
    pub by_text: Vec<TextLength>,
}

/// One speaker's utterances in one text, with what the text says of when
/// it was recorded and how old the speaker was.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextLength {
    pub text: String,
    pub title: String,
    /// The text's `age_<track>` detail (CHAT's `@ID` age, `2;06.15`), else empty.
    pub age: String,
    /// The text's date detail, else empty.
    pub date: String,
    pub utterances: usize,
    pub words: usize,
    pub analysed: usize,
    pub morphemes: usize,
}

/// Details a text's recording date may be under.
const DATE_KEYS: &[&str] = &["date", "recorded", "recording_date"];

/// Fewer running words than this give no MTLD: too few factors to mean anything.
pub const MTLD_MIN: usize = 100;
/// The type/token ratio at which a factor closes (McCarthy and Jarvis 2010).
const MTLD_TTR: f64 = 0.72;

/// One direction of MTLD: running words over factors, where a factor closes
/// once the type/token ratio since the last one falls to 0.72 and the words
/// left over count as part of a factor. None when no factor ever forms.
fn mtld_pass(words: impl Iterator<Item = u32>) -> Option<f64> {
    let mut seen: HashSet<u32> = HashSet::new();
    let (mut count, mut total, mut factors) = (0usize, 0usize, 0.0f64);
    for w in words {
        total += 1;
        count += 1;
        seen.insert(w);
        if seen.len() as f64 / count as f64 <= MTLD_TTR {
            factors += 1.0;
            seen.clear();
            count = 0;
        }
    }
    if count > 0 {
        factors += (1.0 - seen.len() as f64 / count as f64) / (1.0 - MTLD_TTR);
    }
    (factors > 0.0).then(|| total as f64 / factors)
}

/// MTLD of a run of words (as ids), forwards and backwards averaged.
pub fn mtld(words: &[u32]) -> Option<f64> {
    Some((mtld_pass(words.iter().copied())? + mtld_pass(words.iter().rev().copied())?) / 2.0)
}

/// Morphemes in a CHAT `%mor` tier, as CLAN's MLU counts them.
pub fn mor_morphemes(mor: &str) -> usize {
    let mut n = 0;
    for item in mor.split_whitespace() {
        // Punctuation (`.`, `?`) and terminators carry no `|`.
        for part in item.split(['~', '$']) {
            let Some((_, rest)) = part.split_once('|') else { continue };
            // A compound (`n|+n|ice+n|cream`) is one word; count its first stem only.
            n += 1 + rest.matches('-').count() + part.split_once('|').map_or(0, |(pre, _)| pre.matches('#').count());
        }
    }
    n
}

/// A word CHAT marks as unintelligible or untranscribed.
fn is_placeholder(w: &str) -> bool {
    matches!(w, "xxx" | "yyy" | "www")
}

/// A track's speaker (its `speaker` link to a live person): their name, sex
/// and age as the People panel holds them; all empty when it has none.
fn person_of(p: &wugs_core::Project, tier: &str) -> Result<(String, String, String, String), String> {
    let Some(person) = p.proj.field_text(tier, "speaker")? else {
        return Ok(Default::default());
    };
    if p.proj.is_tombstoned(&person)? {
        return Ok(Default::default());
    }
    let name = p.proj.field(&person, "names")?.and_then(|(v, _)| v.get_str("full").map(str::to_string)).unwrap_or_default();
    let details = p.person_details(&person)?;
    let get = |key: &str| details.iter().find(|(k, _)| k == key).map(|(_, v)| v.trim().to_string()).unwrap_or_default();
    Ok((name, get("sex"), get("age"), get("born")))
}

pub fn speakers(session: &mut Session) -> Result<Vec<SpeakerRow>, String> {
    let p = project(session)?;
    let mut rows: Vec<SpeakerRow> = Vec::new();
    let mut texts: HashMap<String, HashSet<String>> = HashMap::new();
    let mut types: HashMap<String, HashSet<String>> = HashMap::new();
    let mut ids: HashMap<String, u32> = HashMap::new();
    let mut runs: HashMap<String, Vec<u32>> = HashMap::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        let words: Vec<String> = seg.words.iter().map(|w| wugs_model::fold_key(&p.proj.nfc(w))).filter(|k| !k.is_empty() && !is_placeholder(k)).collect();
        if words.is_empty() {
            return Ok(());
        }
        let at = match rows.iter().position(|r| r.tier == seg.tier) {
            Some(i) => i,
            None => {
                let name = p.proj.field_str(&seg.tier, "name")?.unwrap_or_default();
                let (person, sex, age, born) = person_of(p, &seg.tier)?;
                rows.push(SpeakerRow { tier: seg.tier.clone(), name, texts: 0, utterances: 0, words: 0, types: 0, longest: 0, analysed: 0, morphemes: 0, mtld: None, person, sex, age, born, by_text: Vec::new() });
                rows.len() - 1
            }
        };
        let row = &mut rows[at];
        row.utterances += 1;
        row.words += words.len();
        let per = match row.by_text.iter().position(|t| t.text == seg.text) {
            Some(i) => i,
            None => {
                row.by_text.push(TextLength { text: seg.text.clone(), title: String::new(), age: String::new(), date: String::new(), utterances: 0, words: 0, analysed: 0, morphemes: 0 });
                row.by_text.len() - 1
            }
        };
        row.by_text[per].utterances += 1;
        row.by_text[per].words += words.len();
        row.longest = row.longest.max(words.len());
        texts.entry(seg.tier.clone()).or_default().insert(seg.text.clone());
        let run = runs.entry(seg.tier.clone()).or_default();
        for w in &words {
            let next = ids.len() as u32;
            run.push(*ids.entry(w.clone()).or_insert(next));
        }
        types.entry(seg.tier.clone()).or_default().extend(words);
        let mut glossed = 0;
        let mut morphs = 0;
        for t in &seg.tokens {
            let n = p.proj.members(t)?.len();
            if n > 0 {
                glossed += 1;
                morphs += n;
            }
        }
        let known = if !seg.tokens.is_empty() && glossed == seg.tokens.len() {
            morphs
        } else if let Some(mor) = p.proj.field_str(&seg.id, "x.chat.mor")? {
            mor_morphemes(&mor)
        } else {
            0
        };
        if known > 0 {
            row.analysed += 1;
            row.morphemes += known;
            row.by_text[per].analysed += 1;
            row.by_text[per].morphemes += known;
        }
        Ok(())
    })?;
    for r in &mut rows {
        r.texts = texts.get(&r.tier).map_or(0, HashSet::len);
        r.types = types.get(&r.tier).map_or(0, HashSet::len);
        r.mtld = runs.get(&r.tier).filter(|run| run.len() >= MTLD_MIN).and_then(|run| mtld(run));
        let age_key = wugs_core::metadata::key_of(&format!("age {}", r.name));
        for t in &mut r.by_text {
            t.title = p.proj.field_str(&t.text, "title")?.unwrap_or_default();
            let details = p.text_metadata(&t.text)?;
            let get = |k: &str| details.iter().find(|(dk, _)| dk == k).map(|(_, v)| v.trim().to_string());
            t.age = age_key.as_deref().and_then(get).unwrap_or_default();
            t.date = DATE_KEYS.iter().find_map(|k| get(k)).unwrap_or_default();
        }
        r.by_text.sort_by(|a, b| a.title.cmp(&b.title));
    }
    rows.sort_by(|a, b| b.utterances.cmp(&a.utterances).then_with(|| a.name.cmp(&b.name)));
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session;
    use std::fs;

    #[test]
    fn mor_counts_suffixes_prefixes_and_clitics() {
        assert_eq!(mor_morphemes("pro:dem|that cop|be&3S det:art|a n|dog-PL ."), 5);
        assert_eq!(mor_morphemes("pro|it~v|be&3S un#v|tie-PRESP ?"), 5, "a clitic, a prefix and a suffix");
        assert_eq!(mor_morphemes("n|+n|ice+n|cream ."), 1, "a compound is one");
        assert_eq!(mor_morphemes(". ?"), 0);
    }

    #[test]
    fn mtld_by_hand() {
        // a b a | b a b: the ratio falls to 2/3 after three words, twice each way.
        assert_eq!(mtld(&[0, 1, 0, 1, 0, 1]), Some(3.0));
        // a b a | b: the last word alone is a ratio of 1, no part of a factor.
        assert_eq!(mtld(&[0, 1, 0, 1]), Some(4.0));
        // Never falling: no factor, no measure.
        assert_eq!(mtld(&[0, 1, 2, 3]), None);
        // Part of the way down to 0.72 is that part of a factor: 10/13 is 0.77.
        let p = mtld_pass([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2].into_iter()).unwrap();
        assert!((p - 13.0 / ((1.0 - 10.0 / 13.0) / 0.28)).abs() < 1e-9, "{p}");
        // One word more and the ratio is 10/14, 0.714: a whole factor.
        assert_eq!(mtld_pass([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3].into_iter()), Some(14.0));
    }

    #[test]
    fn each_speaker_gets_their_own_lengths() {
        let dir = std::env::temp_dir().join(format!("wugs-mlu-{}", wugs_model::ulid()));
        let mut s: session::Session = None;
        session::create_project(&mut s, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let file = dir.join("lunch.cha");
        fs::write(&file, "@Begin\n*MOT:\twhat is that ?\n%mor:\tpro:int|what cop|be&3S pro:dem|that ?\n*CHI:\tdoggies .\n%mor:\tn|doggie-PL .\n*CHI:\tbig xxx doggie .\n*MOT:\tyes , a big doggie .\n@End\n").unwrap();
        crate::interchange::import_file(&mut s, file.to_str().unwrap()).unwrap();
        let rows = speakers(&mut s).unwrap();
        let get = |n: &str| rows.iter().find(|r| r.name == n).unwrap().clone();
        let mot = get("MOT");
        assert_eq!((mot.utterances, mot.words, mot.longest, mot.analysed, mot.morphemes, mot.texts), (2, 7, 4, 1, 3, 1));
        let chi = get("CHI");
        assert_eq!((chi.utterances, chi.words, chi.types, chi.analysed, chi.morphemes), (2, 3, 3, 1, 2), "xxx is no word; one utterance has no %mor");
        // The corpus tools narrowed to one speaker's track.
        let tracks = crate::concordance::tracks(&mut s).unwrap();
        let chi_track = tracks.iter().find(|t| t.name == "CHI").unwrap().id.clone();
        let only_chi = crate::concordance::Scope::of(None, Some(crate::concordance::Where { tier: Some(chi_track), ..Default::default() }));
        let all = crate::concordance::concordance_in(&mut s, "doggie", crate::concordance::Kind::Word, crate::concordance::Match::Exact, &crate::concordance::Scope::all()).unwrap();
        let theirs = crate::concordance::concordance_in(&mut s, "doggie", crate::concordance::Kind::Word, crate::concordance::Match::Exact, &only_chi).unwrap();
        assert_eq!((all.total, theirs.total), (2, 1), "the mother's doggie is not the child's");
        // The child's words against the mother's.
        let k = crate::keywords::keywords(&mut s, crate::concordance::Kind::Word, &only_chi, 1).unwrap();
        assert_eq!((k.focus_tokens, k.reference_tokens), (4, 7), "the corpus tools count xxx as written");
        // The child's track spoken by someone in the People panel.
        assert_eq!((chi.person.as_str(), chi.sex.as_str()), ("", ""));
        let p = session::project(&mut s).unwrap();
        let person = p.add_named_person("Lena", &["child".to_string()]).unwrap();
        p.set_person_detail(&person, "sex", "female").unwrap();
        p.set_person_detail(&person, "age", " 2;06 ").unwrap();
        let chi_track = tracks.iter().find(|t| t.name == "CHI").unwrap().id.clone();
        p.set_tier_speaker(&chi_track, Some(&person)).unwrap();
        let rows = speakers(&mut s).unwrap();
        let chi = rows.iter().find(|r| r.name == "CHI").unwrap();
        assert_eq!((chi.person.as_str(), chi.sex.as_str(), chi.age.as_str()), ("Lena", "female", "2;06"));
        assert_eq!(rows.iter().find(|r| r.name == "MOT").unwrap().person, "");
        // The lunch transcript has no ages: one text, nothing to place in time.
        let chi = rows.iter().find(|r| r.name == "CHI").unwrap();
        assert_eq!(chi.by_text.len(), 1);
        assert_eq!((chi.by_text[0].utterances, chi.by_text[0].words, chi.by_text[0].analysed, chi.by_text[0].morphemes, chi.by_text[0].age.as_str()), (2, 3, 1, 2, ""));
        // Two more sessions, each with the child's age in its @ID line.
        for (name, age, line) in [("bath.cha", "2;03.12", "*CHI:\tmore bubbles .\n%mor:\tqn|more n|bubble-PL .\n"), ("park.cha", "2;09.01", "*CHI:\tthe doggie is running .\n%mor:\tdet|the n|doggie aux|be&3S part|run-PRESP .\n")] {
            let file = dir.join(name);
            fs::write(&file, format!("@Begin\n@Participants:\tCHI Target_Child, MOT Mother\n@ID:\teng|test|CHI|{age}|female|||Target_Child|||\n@ID:\teng|test|MOT|||||Mother|||\n@Date:\t12-JAN-1990\n{line}@End\n")).unwrap();
            crate::interchange::import_file(&mut s, file.to_str().unwrap()).unwrap();
        }
        let rows = speakers(&mut s).unwrap();
        let chi = rows.iter().find(|r| r.name == "CHI").unwrap();
        let by: Vec<(&str, &str, usize, usize)> = chi.by_text.iter().map(|t| (t.title.as_str(), t.age.as_str(), t.utterances, t.morphemes)).collect();
        assert_eq!(by, vec![("bath", "2;03.12", 1, 3), ("lunch", "", 2, 2), ("park", "2;09.01", 1, 5)]);
        assert_eq!(chi.by_text[0].date, "1990-01-12");
        assert_eq!((chi.utterances, chi.morphemes), (4, 10), "the totals take in every text");
        drop(s);
        fs::remove_dir_all(dir).ok();
    }
}
