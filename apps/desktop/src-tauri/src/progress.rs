//! How far the documentation has come, text by text: what a grant report,
//! a deposit form or a supervisor asks for (hours recorded, how much is
//! transcribed, glossed and translated, how big the lexicon is). Read-only,
//! counted the way the corpus tools count (`concordance::each_segment`).

use std::collections::HashMap;

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{project, Session};

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextProgress {
    pub id: String,
    pub title: String,
    /// Length of the text's recording, when it has one on this computer.
    pub recording_ms: Option<f64>,
    /// Segments on transcription tracks.
    pub sentences: usize,
    /// Those with a transcription.
    pub transcribed: usize,
    /// Time covered by transcribed timed segments.
    pub transcribed_ms: f64,
    pub words: usize,
    /// Words broken into morphs.
    pub glossed_words: usize,
    /// The morphs of those words: over `glossed_words`, the index of synthesis.
    pub morphs: usize,
    /// Transcribed sentences with a translation.
    pub translated: usize,
    /// The text's details (`meta:<key>`), for the corpus's make-up by detail.
    pub meta: Vec<(String, String)>,
    /// How varied its words are (MTLD, `mlu::mtld`), from 100 words up.
    pub mtld: Option<f64>,
    /// Its sharing level (`text_access`: its own, else its recording's, else open).
    pub access: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub texts: Vec<TextProgress>,
    /// Every recording linked to a text, each counted once.
    pub recorded_ms: f64,
    /// Recordings linked to a text whose audio is not on this computer.
    pub recordings_missing: usize,
    pub entries: usize,
    pub senses: usize,
    pub senses_without_gloss: usize,
    pub people: usize,
    pub consent_records: usize,
    /// MTLD over every text's words, read text after text.
    pub mtld: Option<f64>,
}

/// A WAV's length from its header, without reading its samples.
fn wav_ms(path: &std::path::Path) -> Option<f64> {
    let file = std::fs::File::open(path).ok()?;
    let reader = wugs_peaks::WavReader::new(std::io::BufReader::new(file)).ok()?;
    let frames = reader.declared_frames()?;
    let rate = reader.format().sample_rate;
    (rate > 0).then(|| frames as f64 * 1000.0 / f64::from(rate))
}

fn live(p: &wugs_core::Project, kind: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for id in p.proj.entities_of_kind(kind)? {
        if !p.proj.is_tombstoned(&id)? {
            out.push(id);
        }
    }
    Ok(out)
}

pub fn progress(session: &mut Session) -> Result<Progress, String> {
    let p = project(session)?;
    let mut by_text: HashMap<String, TextProgress> = HashMap::new();
    // Each text's running words as ids, for MTLD; folded as the corpus tools count them.
    let mut ids: HashMap<String, u32> = HashMap::new();
    let mut runs: HashMap<String, Vec<u32>> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        let row = by_text.entry(seg.text.clone()).or_insert_with(|| TextProgress { id: seg.text.clone(), title: seg.title.clone(), ..TextProgress::default() });
        row.sentences += 1;
        if seg.words.is_empty() {
            return Ok(());
        }
        row.transcribed += 1;
        row.words += seg.words.len();
        let run = runs.entry(seg.text.clone()).or_insert_with(|| {
            order.push(seg.text.clone());
            Vec::new()
        });
        for w in &seg.words {
            let k = wugs_model::fold_key(&p.proj.nfc(w));
            if k.is_empty() {
                continue;
            }
            let next = ids.len() as u32;
            run.push(*ids.entry(k).or_insert(next));
        }
        for token in &seg.tokens {
            let morphs = p.proj.members(token)?.len();
            if morphs > 0 {
                row.glossed_words += 1;
                row.morphs += morphs;
            }
        }
        if let (Some(start), Some(end)) = (p.proj.field_num(&seg.id, "start")?, p.proj.field_num(&seg.id, "end")?) {
            row.transcribed_ms += (end - start).max(0.0);
        }
        if !crate::interchange::translations_of(p, &seg.id)?.is_empty() {
            row.translated += 1;
        }
        Ok(())
    })?;
    let mut out = Progress::default();
    let mut lengths: HashMap<String, Option<f64>> = HashMap::new();
    for text in live(p, "text")? {
        let mut row = by_text.remove(&text).unwrap_or_else(|| TextProgress { id: text.clone(), ..TextProgress::default() });
        if row.title.is_empty() {
            row.title = p.proj.field_str(&text, "title")?.unwrap_or_default();
        }
        row.meta = p.text_metadata(&text)?;
        row.access = p.text_access(&text)?;
        row.mtld = runs.get(&text).filter(|r| r.len() >= crate::mlu::MTLD_MIN).and_then(|r| crate::mlu::mtld(r));
        if let Some((_, blob)) = p.text_recording(&text)? {
            let length = lengths
                .entry(blob.clone())
                .or_insert_with(|| wugs_blob::parse_ref(&blob).and_then(|hex| wav_ms(&p.root.join("blobs").join(hex.to_string()))))
                .to_owned();
            row.recording_ms = length;
        }
        out.texts.push(row);
    }
    out.texts.sort_by_cached_key(|t| (wugs_model::fold_key(&t.title), t.id.clone()));
    out.recorded_ms = lengths.values().flatten().sum();
    let all: Vec<u32> = order.iter().filter_map(|t| runs.get(t)).flatten().copied().collect();
    out.mtld = (all.len() >= crate::mlu::MTLD_MIN).then(|| crate::mlu::mtld(&all)).flatten();
    out.recordings_missing = lengths.values().filter(|l| l.is_none()).count();
    let entries = live(p, "lexeme")?;
    out.entries = entries.len();
    for lexeme in &entries {
        for sense in p.proj.ordered_children("lexeme", lexeme, "sense")? {
            out.senses += 1;
            if p.proj.field_str(&sense, "gloss")?.unwrap_or_default().trim().is_empty() {
                out.senses_without_gloss += 1;
            }
        }
    }
    out.people = live(p, "person")?.len();
    out.consent_records = live(p, "consent")?.len();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tutorial_counts_as_it_was_built() {
        let dir = std::env::temp_dir().join(format!("wugs-progress-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        let r = progress(&mut session).unwrap();
        assert_eq!(r.texts.len(), 1);
        let t = &r.texts[0];
        // «wʌn tuː θɹiː», «fɔɹ faɪv» transcribed, glossed and translated; «sɪks» left to do.
        assert_eq!((t.transcribed, t.words, t.glossed_words, t.morphs), (2, 5, 5, 5), "{t:?}");
        assert!(t.sentences >= 3, "{t:?}");
        assert_eq!(t.translated, 2);
        let rec = t.recording_ms.expect("the tutorial recording is here");
        assert!(rec > 3000.0 && t.transcribed_ms > 0.0 && t.transcribed_ms < rec, "{t:?}");
        assert_eq!(r.recorded_ms, rec);
        assert!(r.entries >= 5 && r.senses >= r.entries && r.senses_without_gloss == 0, "{r:?}");
        assert_eq!((t.mtld, r.mtld), (None, None), "five words are too few to measure how varied they are");
        // A long enough text gets one: a text of 120 words from a small vocabulary.
        let words: Vec<String> = (0..12).map(|i| (0..10).map(|j| ["ka", "ti", "mo", "su", "ne", "ra", "lo", "pe"][(i * 3 + j * 5) % 8]).collect::<Vec<_>>().join(" ")).collect();
        let p = crate::session::project(&mut session).unwrap();
        let utt = crate::session::find_utterance_tier(p).unwrap().unwrap();
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        p.import_written_text("Long", &utt, &refs).unwrap();
        let r2 = progress(&mut session).unwrap();
        let long = r2.texts.iter().find(|x| x.title == "Long").unwrap();
        assert!(long.mtld.is_some_and(|m| m > 1.0 && m < 120.0), "{:?}", long.mtld);
        assert!(r2.mtld.is_some());
        // A text's details come with its row, for the corpus by detail.
        let text = t.id.clone();
        crate::session::project(&mut session).unwrap().set_text_metadata(&text, "genre", "counting").unwrap();
        assert_eq!(progress(&mut session).unwrap().texts[0].meta, vec![("genre".to_string(), "counting".to_string())]);
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
