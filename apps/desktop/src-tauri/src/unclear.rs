//! Places a transcriber marked as unclear (the Checks panel's Unclear
//! passages): `(?)`, `[unclear]`, `(inaudible)`, CHAT's `xxx` and `yyy`, a
//! gap written `[...]`, and runs of question marks. Listed with their times,
//! to take back to a speaker at the next session. A lone `?` inside a word is
//! left alone, since many transcriptions write a glottal stop with it, and so
//! is a question mark that ends a question.

use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{project, Session};

/// At most this many segments are listed; `total` has every one.
const MAX_LISTED: usize = 1000;

fn marker() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)[\(\[<{]\s*(?:\?+|unclear|inaudible|unintelligible|incomprehensible|unsure|uncertain|not sure|\.\.\.|…)[^\)\]>}]{0,60}[\)\]>}]|\b(?:xxx|yyy)\b|\?{2,}").expect("a valid pattern")
    })
}

/// The unclear marks in a transcription, as written, in order.
pub fn markers_in(content: &str) -> Vec<String> {
    marker().find_iter(content).map(|m| m.as_str().to_string()).collect()
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UnclearRow {
    pub text_id: String,
    pub text_title: String,
    pub tier_id: String,
    pub segment_id: String,
    pub content: String,
    pub start_ms: Option<f64>,
    pub end_ms: Option<f64>,
    pub markers: Vec<String>,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Unclear {
    /// Transcribed segments looked at.
    pub checked: usize,
    /// Segments with a mark.
    pub total: usize,
    pub rows: Vec<UnclearRow>,
}

pub fn unclear(session: &mut Session) -> Result<Unclear, String> {
    let p = project(session)?;
    let mut out = Unclear::default();
    each_segment(p, Read::Words, &Scope::all(), |seg| {
        let content = p.proj.field_str(&seg.id, "content")?.unwrap_or_default();
        if content.trim().is_empty() {
            return Ok(());
        }
        out.checked += 1;
        let markers = markers_in(&content);
        if markers.is_empty() {
            return Ok(());
        }
        out.total += 1;
        if out.rows.len() < MAX_LISTED {
            out.rows.push(UnclearRow {
                text_id: seg.text.clone(),
                text_title: seg.title.clone(),
                tier_id: seg.tier.clone(),
                segment_id: seg.id.clone(),
                content: content.trim().to_string(),
                start_ms: p.proj.field_num(&seg.id, "start")?,
                end_ms: p.proj.field_num(&seg.id, "end")?,
                markers,
            });
        }
        Ok(())
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_are_found_and_questions_and_glottal_stops_left_alone() {
        assert_eq!(markers_in("ngai toruma (?) kesi"), vec!["(?)"]);
        assert_eq!(markers_in("he went [unclear: to the river?] and xxx came"), vec!["[unclear: to the river?]", "xxx"]);
        assert_eq!(markers_in("[Inaudible] then (...) yes"), vec!["[Inaudible]", "(...)"]);
        assert_eq!(markers_in("what did he say??"), vec!["??"]);
        assert!(markers_in("did he go?").is_empty());
        assert!(markers_in("ma?a ta?u").is_empty());
        assert!(markers_in("the boxxxer").is_empty());
        assert!(markers_in("(laughs) he said").is_empty());
    }

    #[test]
    fn unclear_segments_are_listed_with_their_times() {
        let dir = std::env::temp_dir().join(format!("wugs-unclear-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut session: Session = None;
        crate::session::create_project(&mut session, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let text = crate::session::create_text(&mut session, "Story").unwrap();
        crate::session::add_segment(&mut session, &text, None, Some(0.0), Some(900.0), "the fox (?) ran").unwrap();
        crate::session::add_segment(&mut session, &text, None, Some(1000.0), Some(1900.0), "did it run?").unwrap();
        crate::session::add_segment(&mut session, &text, None, Some(2000.0), Some(2900.0), "xxx the well").unwrap();
        let u = unclear(&mut session).unwrap();
        assert_eq!((u.checked, u.total), (3, 2));
        assert_eq!(u.rows[0].markers, vec!["(?)"]);
        assert_eq!((u.rows[1].start_ms, u.rows[1].end_ms), (Some(2000.0), Some(2900.0)));
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
