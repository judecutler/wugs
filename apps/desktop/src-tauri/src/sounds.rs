//! The words the Sounds panel reads: every word type on the transcription
//! tracks (one track, or all), with how often it occurs. The panel cuts the
//! words into sounds and places them on the IPA chart itself
//! (`src/sounds/ipa.ts`), so the rules for what a sound is live in one place
//! and are tested there. Nothing here writes.

use std::collections::HashMap;

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{self, Session};

/// More word types than this are left out, the rarest first.
const MAX_TYPES: usize = 200_000;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub name: String,
    /// Running words on it, in the texts read.
    pub words: usize,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct WordType {
    pub word: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundWords {
    /// Every transcription track with words, fullest first.
    pub tracks: Vec<Track>,
    /// The word types on the chosen track (or all of them), commonest first.
    pub words: Vec<WordType>,
    /// Running words read.
    pub running: usize,
    /// Word types left out past `MAX_TYPES`.
    pub omitted: usize,
}

/// The word types on `tier` (every transcription track when None) of one
/// text, or of every text, exactly as written (NFC; case kept, since IPA
/// tells `ɢ` from `G`).
pub fn sound_words(session: &mut Session, tier: Option<&str>, text: Option<String>) -> Result<SoundWords, String> {
    let p = session::project(session)?;
    let mut per_tier: HashMap<String, usize> = HashMap::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut running = 0;
    each_segment(p, Read::Words, &Scope::of(text, None), |seg| {
        let words = seg.words.iter().filter(|w| !w.trim().is_empty());
        let n = words.clone().count();
        *per_tier.entry(seg.tier.clone()).or_default() += n;
        if tier.is_none_or(|t| t == seg.tier) {
            running += n;
            for w in words {
                *counts.entry(p.proj.nfc(w)).or_default() += 1;
            }
        }
        Ok(())
    })?;
    let mut tracks = Vec::new();
    for (id, words) in per_tier {
        if words == 0 {
            continue;
        }
        let name = p.proj.field_str(&id, "name")?.unwrap_or_default();
        tracks.push(Track { id, name, words });
    }
    tracks.sort_by(|a, b| b.words.cmp(&a.words).then(a.name.cmp(&b.name)));
    let mut words: Vec<WordType> = counts.into_iter().map(|(word, count)| WordType { word, count }).collect();
    words.sort_by(|a, b| b.count.cmp(&a.count).then(a.word.cmp(&b.word)));
    let omitted = words.len().saturating_sub(MAX_TYPES);
    words.truncate(MAX_TYPES);
    Ok(SoundWords { tracks, words, running, omitted })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_types_are_counted_per_track() {
        let dir = std::env::temp_dir().join(format!("wugs-sounds-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let text = session::create_text(&mut s, "words").unwrap();
        for line in ["tʰa ka", "ka ŋa.", "tʰa tʰa"] {
            session::add_segment(&mut s, &text, None, None, None, line).unwrap();
        }
        let all = sound_words(&mut s, None, None).unwrap();
        assert_eq!(all.running, 6);
        assert_eq!(all.words[0], WordType { word: "tʰa".into(), count: 3 });
        assert_eq!(all.words.iter().find(|w| w.word == "ŋa").map(|w| w.count), Some(1), "edge punctuation is not part of a word");
        assert_eq!(all.tracks.len(), 1);
        assert_eq!(all.tracks[0].words, 6);
        let tier = all.tracks[0].id.clone();
        assert_eq!(sound_words(&mut s, Some(&tier), Some(text.clone())).unwrap().running, 6);
        assert_eq!(sound_words(&mut s, Some("tir_none"), None).unwrap().running, 0);
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
