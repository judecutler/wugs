//! Glossing an ELAN file keeps in its own tiers (words, morphs, glosses),
//! read into the text's tokens and morphs on request (`wugs_elan::glossing`).
//! Asked, not done at import: a Time_Subdivision word tier's times are not
//! kept by tokens, and someone may want the tiers as they are.

use serde::Serialize;

use crate::session::{project, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TierGlossing {
    /// Which layout of the text, for `read`.
    pub index: usize,
    /// Transcription, words, morphs and glosses tiers, those there are.
    pub names: Vec<String>,
    pub has_morphs: bool,
    pub has_glosses: bool,
    /// Tiers whose values are kept on each word or morph.
    pub kept: Vec<String>,
    /// Tiers below the words that go unread.
    pub dropped: Vec<String>,
    /// The word tier is timed (Time_Subdivision or Included_In), and tokens keep no times.
    pub timed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadGlossing {
    pub sentences: usize,
    pub words: usize,
    pub morphs: usize,
    pub glossed: usize,
    pub kept: usize,
    pub stray: usize,
}

pub fn find(session: &mut Session, text: &str) -> Result<Vec<TierGlossing>, String> {
    let p = project(session)?;
    let mut out = Vec::new();
    for (index, l) in wugs_elan::glossing::layouts(p, text)?.into_iter().enumerate() {
        let constraints = p.proj.field_str(&l.words, "x.elan.constraints")?;
        out.push(TierGlossing {
            index,
            names: l.names,
            has_morphs: l.morphs.is_some(),
            has_glosses: l.glosses.is_some(),
            kept: l.kept.into_iter().map(|k| k.1).collect(),
            dropped: l.dropped.into_iter().map(|k| k.1).collect(),
            timed: matches!(constraints.as_deref(), Some("Time_Subdivision" | "Included_In")),
        });
    }
    Ok(out)
}

pub fn read(session: &mut Session, text: &str, index: usize) -> Result<ReadGlossing, String> {
    let p = project(session)?;
    let layout = wugs_elan::glossing::layouts(p, text)?.into_iter().nth(index).ok_or("those tiers are no longer in the text")?;
    let r = wugs_elan::glossing::read_glossing(p, text, &layout)?;
    Ok(ReadGlossing { sentences: r.sentences, words: r.words, morphs: r.morphs, glossed: r.glossed, kept: r.kept, stray: r.stray })
}
