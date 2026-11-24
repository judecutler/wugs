//! The work log: what was done in the project day by day, read from the op
//! log (each op carries its time, its device and the intent that wrote it),
//! for a field diary, a supervisor or a grant report. Nothing is stored:
//! the log already holds it.
//!
//! A day counts each sentence, word, gloss and entry once however often it
//! was edited that day. Work done by hand is told apart from imports
//! (`import-*` intents) and machine suggestions (`machine-*`).

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;
use wugs_model::hlc::Hlc;

use crate::session::{project, Session};

/// What one day holds.
#[derive(Debug, Default, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    /// `YYYY-MM-DD` in the person's own time.
    pub date: String,
    /// Segments made by hand (on the Timeline, by tapping, by splitting).
    pub segments: usize,
    /// Segments whose text someone typed.
    pub transcribed: usize,
    /// Translations added (segments naming a sentence as their parent).
    pub translated: usize,
    /// Words given an analysis (morphs made or retyped).
    pub analysed: usize,
    /// Morphs whose gloss someone typed.
    pub glossed: usize,
    /// Lexicon entries and senses made by hand.
    pub entries: usize,
    pub senses: usize,
    /// Notes on sentences.
    pub notes: usize,
    /// Texts and lexicon entries brought in by an import (`import-*`).
    pub imported_texts: usize,
    pub imported_entries: usize,
    /// Segments a machine wrote or suggested for (`machine-*`).
    pub machine: usize,
    /// Every op of the day, and how many devices wrote them.
    pub ops: usize,
    pub devices: usize,
}

#[derive(Default)]
struct Sets {
    segments: HashSet<String>,
    transcribed: HashSet<String>,
    translated: HashSet<String>,
    analysed: HashSet<String>,
    glossed: HashSet<String>,
    entries: HashSet<String>,
    senses: HashSet<String>,
    notes: HashSet<String>,
    imported_texts: HashSet<String>,
    imported_entries: HashSet<String>,
    machine: HashSet<String>,
    devices: HashSet<String>,
    ops: usize,
}

/// The days with any work, newest first, at most `days` of them, in the
/// time zone `offset_minutes` east of UTC.
pub fn worklog(session: &mut Session, offset_minutes: i32, days: usize) -> Result<Vec<Day>, String> {
    let p = project(session)?;
    let mut by_day: BTreeMap<String, Sets> = BTreeMap::new();
    let mut stream = p.log.stream_all().map_err(|e| e.to_string())?;
    while let Some(op) = stream.next_op().map_err(|e| e.to_string())? {
        let Some(hlc) = Hlc::parse(&op.hlc) else { continue };
        let date = crate::corrections::date_of_secs((hlc.millis as i64) / 1000 + i64::from(offset_minutes) * 60);
        let d = by_day.entry(date).or_default();
        d.ops += 1;
        d.devices.insert(op.device.clone());
        let intent = op.intent.as_deref().unwrap_or("");
        let field = op.field.as_deref().unwrap_or("");
        if intent.starts_with("import") {
            match (op.verb.as_str(), op.entity.as_str()) {
                ("create", "text") => {
                    d.imported_texts.insert(op.target.clone());
                }
                ("create", "lexeme") => {
                    d.imported_entries.insert(op.target.clone());
                }
                _ => {}
            }
            continue;
        }
        if intent.starts_with("machine") {
            if op.entity == "segment" {
                d.machine.insert(op.target.clone());
            }
            continue;
        }
        match (op.verb.as_str(), op.entity.as_str()) {
            ("create", "segment") => {
                if op.value.get_str("parent").is_some_and(|x| !x.is_empty()) {
                    d.translated.insert(op.target.clone());
                } else {
                    d.segments.insert(op.target.clone());
                }
                if op.value.get_str("content").is_some_and(|c| !c.trim().is_empty()) && op.value.get_str("parent").is_none() {
                    d.transcribed.insert(op.target.clone());
                }
            }
            ("set" | "text-edit", "segment") if field == "content" => {
                d.transcribed.insert(op.target.clone());
            }
            ("create", "morph") => {
                if let Some(t) = op.value.get_str("token") {
                    d.analysed.insert(t.to_string());
                }
                if op.value.get_str("gloss").is_some_and(|g| !g.trim().is_empty()) {
                    d.glossed.insert(op.target.clone());
                }
            }
            ("set", "token") if field == "morphs" => {
                d.analysed.insert(op.target.clone());
            }
            ("set", "morph") if field == "gloss" => {
                d.glossed.insert(op.target.clone());
            }
            ("create", "lexeme") => {
                d.entries.insert(op.target.clone());
            }
            ("create", "sense") => {
                d.senses.insert(op.target.clone());
            }
            ("create", "annotation") => {
                d.notes.insert(op.target.clone());
            }
            _ => {}
        }
    }
    Ok(by_day
        .into_iter()
        .rev()
        .take(days)
        .map(|(date, s)| Day {
            date,
            segments: s.segments.len(),
            transcribed: s.transcribed.len(),
            translated: s.translated.len(),
            analysed: s.analysed.len(),
            glossed: s.glossed.len(),
            entries: s.entries.len(),
            senses: s.senses.len(),
            notes: s.notes.len(),
            imported_texts: s.imported_texts.len(),
            imported_entries: s.imported_entries.len(),
            machine: s.machine.len(),
            ops: s.ops,
            devices: s.devices.len(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session;

    #[test]
    fn a_day_of_work_is_counted_once_per_thing() {
        assert_eq!(crate::corrections::date_of_secs(0), "1970-01-01");
        assert_eq!(crate::corrections::date_of_secs(1_763_942_400), "2025-11-24");
        let dir = std::env::temp_dir().join(format!("wugs-worklog-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, dir.to_str().unwrap(), "tester").unwrap();
        let text = session::create_text(&mut s, "Market").unwrap();
        let a = session::add_segment(&mut s, &text, None, Some(0.0), Some(900.0), "").unwrap().id;
        session::add_segment(&mut s, &text, None, Some(1000.0), Some(1900.0), "ka nimo").unwrap();
        session::edit_segment_content(&mut s, &text, &a, "ka").unwrap();
        session::edit_segment_content(&mut s, &text, &a, "ka pela").unwrap();
        crate::lexicon::lexicon_add(&mut s, "nimo", "fish").unwrap();
        let days = worklog(&mut s, 0, 30).unwrap();
        assert_eq!(days.len(), 1, "{days:?}");
        let d = &days[0];
        assert_eq!((d.segments, d.transcribed, d.entries, d.senses, d.devices), (2, 2, 1, 1, 1), "a sentence typed twice counts once: {d:?}");
        assert_eq!(d.date, crate::corrections::today());
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
