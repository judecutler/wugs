//! The Sessions panel's commands: the fieldwork log (`wugs_core::field_sessions`),
//! each session with the names of the people in it and the texts from that
//! day (a text whose date detail, or whose recording's date, is the
//! session's), so a log entry leads to what was recorded. Every command
//! returns the whole view.

use serde::Serialize;

use crate::session::{list_texts, project, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub date: String,
    pub place: String,
    pub method: String,
    pub notes: String,
    /// `(person id, name)`, by name.
    pub people: Vec<(String, String)>,
    /// `(text id, title)` of the texts dated that day, by title.
    pub texts: Vec<(String, String)>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsView {
    pub sessions: Vec<SessionView>,
    /// Everyone in the People panel, `(id, name)` by name, to add to a session.
    pub people: Vec<(String, String)>,
}

/// The keys a text's or recording's date may be under (as the catalogue sheet reads them).
const DATE_KEYS: &[&str] = &["date", "recorded", "recording_date"];

pub fn view(s: &mut Session) -> Result<SessionsView, String> {
    let texts = list_texts(s)?;
    let p = project(s)?;
    let mut people: Vec<(String, String)> = p.people()?.into_iter().filter(|x| !x.deleted).map(|x| (x.id, x.name)).collect();
    people.sort_by_cached_key(|(id, name)| (wugs_model::fold_key(name), id.clone()));
    // Each text's date: its own detail, else its recording's.
    let mut dated: Vec<(String, String, String)> = Vec::new();
    for t in &texts {
        let mut details = p.text_metadata(&t.id)?;
        if let Some((source, _)) = p.text_recording(&t.id)? {
            details.extend(p.source_details(&source)?);
        }
        if let Some(date) = DATE_KEYS.iter().find_map(|k| details.iter().find(|(dk, v)| dk == k && !v.trim().is_empty()).map(|(_, v)| v.trim().to_string())) {
            dated.push((date, t.id.clone(), t.title.clone()));
        }
    }
    let mut sessions = Vec::new();
    for fs in p.field_sessions()? {
        let mut who: Vec<(String, String)> = fs.people.iter().map(|id| (id.clone(), people.iter().find(|(p, _)| p == id).map(|(_, n)| n.clone()).unwrap_or_default())).collect();
        who.sort_by_cached_key(|(id, name)| (wugs_model::fold_key(name), id.clone()));
        let mut day: Vec<(String, String)> = if fs.date.is_empty() { Vec::new() } else { dated.iter().filter(|(d, _, _)| *d == fs.date).map(|(_, id, title)| (id.clone(), title.clone())).collect() };
        day.sort_by_cached_key(|(id, title)| (wugs_model::fold_key(title), id.clone()));
        sessions.push(SessionView { id: fs.id, date: fs.date, place: fs.place, method: fs.method, notes: fs.notes, people: who, texts: day });
    }
    Ok(SessionsView { sessions, people })
}

pub fn add(s: &mut Session, date: &str) -> Result<SessionsView, String> {
    project(s)?.add_field_session(date)?;
    view(s)
}

pub fn set(s: &mut Session, session: &str, field: &str, value: &str) -> Result<SessionsView, String> {
    project(s)?.set_field_session(session, field, value)?;
    view(s)
}

pub fn person(s: &mut Session, session: &str, person: &str, add: bool) -> Result<SessionsView, String> {
    let p = project(s)?;
    if add {
        p.add_session_person(session, person)?;
    } else {
        p.remove_session_person(session, person)?;
    }
    view(s)
}

pub fn delete(s: &mut Session, session: &str) -> Result<SessionsView, String> {
    project(s)?.delete_field_session(session)?;
    view(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_leads_to_the_texts_of_its_day() {
        let dir = std::env::temp_dir().join(format!("wugs-field-sessions-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "prs_Ana").unwrap();
        let text = list_texts(&mut s).unwrap()[0].id.clone();
        let ana = crate::people::add_person(&mut s, "Ana", "speaker").unwrap().people[0].id.clone();
        let v = add(&mut s, "2026-11-03").unwrap();
        let ses = v.sessions[0].id.clone();
        assert!(v.sessions[0].texts.is_empty(), "no text is dated yet");
        // The recording's date dates the text.
        let p = project(&mut s).unwrap();
        let (source, _) = p.text_recording(&text).unwrap().unwrap();
        p.set_source_detail(&source, "date", "2026-11-03").unwrap();
        let v = person(&mut s, &ses, &ana, true).unwrap();
        assert_eq!(v.sessions[0].people, vec![(ana.clone(), "Ana".to_string())]);
        assert_eq!(v.sessions[0].texts.len(), 1);
        assert_eq!(v.sessions[0].texts[0].0, text);
        let v = set(&mut s, &ses, "notes", "Counted to six; ask about seven.").unwrap();
        assert_eq!(v.sessions[0].notes, "Counted to six; ask about seven.");
        assert!(set(&mut s, &ses, "date", "3 Nov").is_err());
        let v = delete(&mut s, &ses).unwrap();
        assert!(v.sessions.is_empty());
        assert_eq!(v.people.len(), 1);
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
