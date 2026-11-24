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
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        let words: Vec<String> = seg.words.iter().map(|w| wugs_model::fold_key(&p.proj.nfc(w))).filter(|k| !k.is_empty() && !is_placeholder(k)).collect();
        if words.is_empty() {
            return Ok(());
        }
        let at = match rows.iter().position(|r| r.tier == seg.tier) {
            Some(i) => i,
            None => {
                let name = p.proj.field_str(&seg.tier, "name")?.unwrap_or_default();
                rows.push(SpeakerRow { tier: seg.tier.clone(), name, texts: 0, utterances: 0, words: 0, types: 0, longest: 0, analysed: 0, morphemes: 0 });
                rows.len() - 1
            }
        };
        let row = &mut rows[at];
        row.utterances += 1;
        row.words += words.len();
        row.longest = row.longest.max(words.len());
        texts.entry(seg.tier.clone()).or_default().insert(seg.text.clone());
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
        drop(s);
        fs::remove_dir_all(dir).ok();
    }
}
