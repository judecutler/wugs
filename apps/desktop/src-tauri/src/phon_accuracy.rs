//! PhonBank's target and actual forms: CHAT's `%mod` (what the speaker
//! was aiming at) and `%pho` (what they said), as CHAT import keeps them
//! (`x.chat.mod`, `x.chat.pho`), handed to the Sounds panel, which scores
//! them (percent consonants correct and the like) in `src/sounds/accuracy.ts`.

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{project, Session};

/// At most this many utterances are handed over.
pub const LIMIT: usize = 50_000;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PhonPair {
    pub track: String,
    pub text: String,
    pub title: String,
    pub segment: String,
    pub target: String,
    pub actual: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PhonPairs {
    pub pairs: Vec<PhonPair>,
    /// Track ids to names.
    pub tracks: Vec<(String, String)>,
    /// Utterances past the limit.
    pub omitted: usize,
}

pub fn pairs(session: &mut Session) -> Result<PhonPairs, String> {
    let p = project(session)?;
    let mut out = PhonPairs { pairs: Vec::new(), tracks: Vec::new(), omitted: 0 };
    each_segment(p, Read::Words, &Scope::all(), |seg| {
        let get = |f: &str| p.proj.field_str(&seg.id, f).map(|v| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
        let (Some(target), Some(actual)) = (get("x.chat.mod")?, get("x.chat.pho")?) else {
            return Ok(());
        };
        if out.pairs.len() >= LIMIT {
            out.omitted += 1;
            return Ok(());
        }
        if !out.tracks.iter().any(|(t, _)| *t == seg.tier) {
            out.tracks.push((seg.tier.clone(), p.proj.field_str(&seg.tier, "name")?.unwrap_or_default()));
        }
        out.pairs.push(PhonPair { track: seg.tier.clone(), text: seg.text.clone(), title: seg.title.clone(), segment: seg.id.clone(), target, actual });
        Ok(())
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session;
    use std::fs;

    #[test]
    fn mod_and_pho_tiers_come_back_paired() {
        let dir = std::env::temp_dir().join(format!("wugs-phon-{}", wugs_model::ulid()));
        let mut s: session::Session = None;
        session::create_project(&mut s, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let file = dir.join("ball.cha");
        fs::write(&file, "@Begin\n*CHI:\tball .\n%mod:\tbɔl\n%pho:\tbɑ\n*CHI:\tdoggie .\n%mod:\tdɔɡi\n*MOT:\tyes .\n@End\n").unwrap();
        crate::interchange::import_file(&mut s, file.to_str().unwrap()).unwrap();
        let r = pairs(&mut s).unwrap();
        assert_eq!(r.pairs.len(), 1, "an utterance needs both tiers");
        assert_eq!((r.pairs[0].target.as_str(), r.pairs[0].actual.as_str(), r.pairs[0].title.as_str()), ("bɔl", "bɑ", "ball"));
        assert_eq!(r.tracks.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>(), vec!["CHI"]);
        drop(s);
        fs::remove_dir_all(dir).ok();
    }
}
