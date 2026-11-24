//! Field sessions: the fieldworker's log of each sitting (DATA-MODEL §3
//! Session): when and where, how it was done (elicitation, recording,
//! transcription with a speaker, checking…), who took part, and notes (what
//! was covered, what to ask next time). A session is an entity of kind
//! `session` in the people scope, the most restricted one, since a log speaks
//! of the people in it; who took part is a `session-person` link per person
//! (as `sense-domain` links senses to domains), so two devices adding people
//! at once both keep theirs. Date, place and method are a person's edits
//! (resolving); notes are collaborative text. A session made by
//! `Project::start_session` (elicitation provenance) lists people in its
//! older `participants` field, a JSON list of person ids or `{person, role}`;
//! they are read as taking part too, and taking one off rewrites that list.

use wugs_model::Value;

use crate::{CommitResult, Project, SCOPE_PEOPLE};

/// The longest notes kept for one session (characters).
pub const MAX_SESSION_NOTES: usize = 20_000;
/// The longest place or method (characters).
const MAX_SHORT: usize = 200;

/// One session as the log shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldSession {
    pub id: String,
    /// `YYYY-MM-DD`, or empty when not given.
    pub date: String,
    pub place: String,
    pub method: String,
    pub notes: String,
    /// The live people who took part, by person id, sorted.
    pub people: Vec<String>,
}

/// Whether `date` is a calendar date written `YYYY-MM-DD`.
pub fn valid_date(date: &str) -> bool {
    let b = date.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) {
        return false;
    }
    let num = |r: std::ops::Range<usize>| date[r].parse::<u32>().unwrap_or(0);
    let (y, m, d) = (num(0..4), num(5..7), num(8..10));
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&d)
}

impl Project {
    fn session_date(date: &str) -> Result<String, String> {
        let date = date.trim();
        if date.is_empty() || valid_date(date) {
            Ok(date.to_string())
        } else {
            Err(format!("«{date}» is not a date written as year-month-day, such as 2026-11-24."))
        }
    }

    fn short(&self, value: &str, what: &str) -> Result<String, String> {
        let v = self.proj.nfc(value.split_whitespace().collect::<Vec<_>>().join(" ").as_str());
        if v.chars().count() > MAX_SHORT {
            return Err(format!("A session's {what} is at most {MAX_SHORT} characters."));
        }
        Ok(v)
    }

    /// Every live session, newest first (undated last), with its live people.
    pub fn field_sessions(&self) -> Result<Vec<FieldSession>, String> {
        let mut out = Vec::new();
        for id in self.proj.entities_of_kind("session")? {
            let mut people: Vec<String> = self.session_people(&id)?.into_iter().map(|(_, p)| p).collect();
            people.dedup();
            let get = |f: &str| -> Result<String, String> { Ok(self.proj.field_str(&id, f)?.unwrap_or_default()) };
            out.push(FieldSession { date: get("date")?, place: get("place")?, method: get("method")?, notes: get("notes")?, people, id });
        }
        out.sort_by(|a, b| (a.date.is_empty(), std::cmp::Reverse(a.date.clone()), std::cmp::Reverse(a.id.clone())).cmp(&(b.date.is_empty(), std::cmp::Reverse(b.date.clone()), std::cmp::Reverse(b.id.clone()))));
        Ok(out)
    }

    /// The people of a session's older `participants` list, as written.
    fn legacy_participants(&self, session: &str) -> Result<Vec<String>, String> {
        let Some((Value::Arr(items), _)) = self.proj.field(session, "participants")? else { return Ok(Vec::new()) };
        Ok(items
            .into_iter()
            .filter_map(|v| match v {
                Value::Str(id) => Some(id),
                Value::Obj(m) => match m.get("person") {
                    Some(Value::Str(id)) => Some(id.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect())
    }

    /// A session's live people: `(link, person)` by person, the link empty
    /// for one named only in the older `participants` list.
    fn session_people(&self, session: &str) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        for person in self.legacy_participants(session)? {
            if self.proj.entity_kind(&person)?.as_deref() == Some("person") && !self.proj.is_tombstoned(&person)? && !out.iter().any(|(_, p): &(String, String)| *p == person) {
                out.push((String::new(), person));
            }
        }
        for link in self.proj.children_by_ref("session", session, "session-person")? {
            if let Some(person) = self.proj.field_str(&link, "person")? {
                if self.proj.entity_kind(&person)?.as_deref() == Some("person") && !self.proj.is_tombstoned(&person)? {
                    out.retain(|(l, p)| !(l.is_empty() && *p == person));
                    out.push((link, person));
                }
            }
        }
        out.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        Ok(out)
    }

    /// Start a session on `date` (`YYYY-MM-DD`, or empty).
    pub fn add_field_session(&mut self, date: &str) -> Result<(String, CommitResult), String> {
        let date = Self::session_date(date)?;
        let mut t = self.intent("add-session");
        let id = t.create("session", "ses", SCOPE_PEOPLE, vec![("date", Value::str(&date))]);
        Ok((id, t.commit()?))
    }

    /// Set a session's `date`, `place`, `method` or `notes` (a person's edit,
    /// so resolving). The same value writes nothing.
    pub fn set_field_session(&mut self, session: &str, field: &str, value: &str) -> Result<Option<CommitResult>, String> {
        self.require(session, "session")?;
        let value = match field {
            "date" => Self::session_date(value)?,
            "place" => self.short(value, "place")?,
            "method" => self.short(value, "method")?,
            "notes" => {
                let v = self.proj.nfc(value.trim());
                if v.chars().count() > MAX_SESSION_NOTES {
                    return Err(format!("A session's notes are at most {MAX_SESSION_NOTES} characters."));
                }
                v
            }
            other => return Err(format!("A session has no field «{other}».")),
        };
        if self.proj.field_str(session, field)?.unwrap_or_default() == value {
            return Ok(None);
        }
        let mut t = self.intent("edit-session").resolving();
        if field == "notes" {
            t.text_edit("session", session, SCOPE_PEOPLE, "notes", &value)?;
        } else {
            t.set("session", session, SCOPE_PEOPLE, field, Value::str(&value))?;
        }
        t.commit().map(Some)
    }

    /// Say that `person` took part in `session` (nothing when they already do).
    pub fn add_session_person(&mut self, session: &str, person: &str) -> Result<CommitResult, String> {
        self.require(session, "session")?;
        self.require(person, "person")?;
        let mut t = self.intent("add-session-person");
        if !t.project.session_people(session)?.iter().any(|(_, p)| p == person) {
            t.create("session-person", "spn", SCOPE_PEOPLE, vec![("session", Value::str(session)), ("person", Value::str(person))]);
        }
        t.commit()
    }

    /// Take `person` off a session: every live link between the two is tombstoned.
    pub fn remove_session_person(&mut self, session: &str, person: &str) -> Result<CommitResult, String> {
        self.require(session, "session")?;
        let links: Vec<String> = self.session_people(session)?.into_iter().filter(|(l, p)| p == person && !l.is_empty()).map(|(l, _)| l).collect();
        let legacy = match self.proj.field(session, "participants")? {
            Some((Value::Arr(items), _)) if self.legacy_participants(session)?.iter().any(|p| p == person) => Some(
                items
                    .into_iter()
                    .filter(|v| match v {
                        Value::Str(id) => id != person,
                        Value::Obj(m) => m.get("person") != Some(&Value::str(person)),
                        _ => true,
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        };
        let mut t = self.intent("remove-session-person").resolving();
        for link in links {
            t.delete("session-person", &link, SCOPE_PEOPLE);
        }
        if let Some(rest) = legacy {
            t.set("session", session, SCOPE_PEOPLE, "participants", Value::Arr(rest))?;
        }
        t.commit()
    }

    /// Delete a session and its people links.
    pub fn delete_field_session(&mut self, session: &str) -> Result<CommitResult, String> {
        self.require(session, "session")?;
        let links: Vec<String> = self.session_people(session)?.into_iter().map(|(l, _)| l).filter(|l| !l.is_empty()).collect();
        let mut t = self.intent("delete-session");
        for link in links {
            t.delete("session-person", &link, SCOPE_PEOPLE);
        }
        t.delete("session", session, SCOPE_PEOPLE);
        t.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_calendar_dates() {
        assert!(valid_date("2026-11-24"));
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2026-02-29"));
        assert!(!valid_date("2026-13-01"));
        assert!(!valid_date("24/11/2026"));
        assert!(!valid_date("2026-1-24"));
    }

    #[test]
    fn a_session_log_is_kept_edited_and_replayed() {
        let root = std::env::temp_dir().join(format!("wugs-sessions-{}", wugs_model::ulid()));
        let mut p = Project::open(&root, "prs_Tester").unwrap();
        let ana = p.add_named_person("Ana", &["speaker".to_string()]).unwrap();
        let ben = p.add_named_person("Ben", &["linguist".to_string()]).unwrap();
        assert!(p.add_field_session("Tuesday").is_err());
        let (first, _) = p.add_field_session("2026-11-20").unwrap();
        let (second, _) = p.add_field_session(" 2026-11-24 ").unwrap();
        let (undated, _) = p.add_field_session("").unwrap();
        let order: Vec<String> = p.field_sessions().unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(order, vec![second.clone(), first.clone(), undated.clone()], "newest first, undated last");
        p.set_field_session(&second, "place", "  Village   hall ").unwrap().unwrap();
        assert!(p.set_field_session(&second, "place", "Village hall").unwrap().is_none(), "unchanged writes nothing");
        p.set_field_session(&second, "method", "elicitation").unwrap();
        p.set_field_session(&second, "notes", "Body parts. Ask about «kani» next time?").unwrap();
        assert!(p.set_field_session(&second, "date", "2026-11-31").is_err());
        assert!(p.set_field_session(&second, "colour", "x").is_err());
        p.add_session_person(&second, &ana).unwrap();
        p.add_session_person(&second, &ana).unwrap();
        p.add_session_person(&second, &ben).unwrap();
        let s = p.field_sessions().unwrap().into_iter().find(|s| s.id == second).unwrap();
        assert_eq!((s.date.as_str(), s.place.as_str(), s.method.as_str()), ("2026-11-24", "Village hall", "elicitation"));
        assert_eq!(s.notes, "Body parts. Ask about «kani» next time?");
        let mut both = vec![ana.clone(), ben.clone()];
        both.sort();
        assert_eq!(s.people, both, "a person once, however often added");
        p.remove_session_person(&second, &ben).unwrap();
        assert_eq!(p.field_sessions().unwrap().into_iter().find(|s| s.id == second).unwrap().people, vec![ana.clone()]);
        p.delete_field_session(&undated).unwrap();
        // A session started for elicitation provenance names its people in `participants`.
        let old = p.start_session("2026-11-01", "Town", "elicitation", Value::Arr(vec![Value::str(&ben)])).unwrap();
        assert_eq!(p.field_sessions().unwrap().into_iter().find(|s| s.id == old).unwrap().people, vec![ben.clone()]);
        p.remove_session_person(&old, &ben).unwrap();
        assert!(p.field_sessions().unwrap().into_iter().find(|s| s.id == old).unwrap().people.is_empty());
        p.delete_field_session(&old).unwrap();
        assert_eq!(p.field_sessions().unwrap().len(), 2);
        drop(p);
        // The log is the truth: a replay gives the same sessions.
        std::fs::remove_dir_all(root.join("cache")).ok();
        let p = Project::open(&root, "prs_Tester").unwrap();
        let s = p.field_sessions().unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].people, vec![ana]);
        assert_eq!(s[0].notes, "Body parts. Ask about «kani» next time?");
        std::fs::remove_dir_all(&root).ok();
    }
}
