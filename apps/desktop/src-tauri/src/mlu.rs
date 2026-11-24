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
}

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
                rows.push(SpeakerRow { tier: seg.tier.clone(), name, texts: 0, utterances: 0, words: 0, types: 0, longest: 0, analysed: 0, morphemes: 0, mtld: None });
                rows.len() - 1
            }
        };
        let row = &mut rows[at];
        row.utterances += 1;
        row.words += words.len();
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
        if !seg.tokens.is_empty() && glossed == seg.tokens.len() {
            row.analysed += 1;
            row.morphemes += morphs;
        } else if let Some(mor) = p.proj.field_str(&seg.id, "x.chat.mor")? {
            let n = mor_morphemes(&mor);
            if n > 0 {
                row.analysed += 1;
                row.morphemes += n;
            }
        }
        Ok(())
    })?;
    for r in &mut rows {
        r.texts = texts.get(&r.tier).map_or(0, HashSet::len);
        r.types = types.get(&r.tier).map_or(0, HashSet::len);
        r.mtld = runs.get(&r.tier).filter(|run| run.len() >= MTLD_MIN).and_then(|run| mtld(run));
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
        let only_chi = crate::concordance::Scope::of(None, Some(crate::concordance::Where { key: String::new(), value: String::new(), tier: Some(chi_track) }));
        let all = crate::concordance::concordance_in(&mut s, "doggie", crate::concordance::Kind::Word, crate::concordance::Match::Exact, &crate::concordance::Scope::all()).unwrap();
        let theirs = crate::concordance::concordance_in(&mut s, "doggie", crate::concordance::Kind::Word, crate::concordance::Match::Exact, &only_chi).unwrap();
        assert_eq!((all.total, theirs.total), (2, 1), "the mother's doggie is not the child's");
        // The child's words against the mother's.
        let k = crate::keywords::keywords(&mut s, crate::concordance::Kind::Word, &only_chi, 1).unwrap();
        assert_eq!((k.focus_tokens, k.reference_tokens), (4, 7), "the corpus tools count xxx as written");
        drop(s);
        fs::remove_dir_all(dir).ok();
    }
}
