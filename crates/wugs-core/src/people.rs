//! People and consent records (DATA-MODEL §3, roadmap item 61): the
//! optional, quick way to write down who is in a project and what each
//! agreed to. Records inform and set defaults; nothing here blocks work
//! (DECISIONS 2026-11-11, "Consent records inform, never enforce").
//!
//! Everything lives in the people scope, the most restricted one. Edits of
//! a field are a person's direct edits, so they are `resolving()`; creates
//! and deletes stay plain, like the lexicon's.

use wugs_model::Value;

use crate::{CommitResult, Project, SCOPE_PEOPLE};

/// The terms the People panel offers. A record may hold others (an
/// importer's, a later version's); they are kept as they are.
pub const CONSENT_TERMS: &[&str] = &["archive-ok", "web-ok", "community-only", "AI-ok"];

/// A person as the panel shows them.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonRow {
    pub id: String,
    /// The `full` name, else the first name of any kind in `names`.
    pub name: String,
    pub roles: Vec<String>,
    pub consents: Vec<ConsentRow>,
}

/// A consent record. `terms` holds only the panel's terms that the record
/// answers: `Some(true)` agreed, `Some(false)` refused, absent not asked.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsentRow {
    pub id: String,
    pub covers: String,
    pub terms: Vec<(String, bool)>,
    pub date: String,
    /// The scan or photo of the signed form, a blob ref.
    pub evidence: Option<String>,
    pub evidence_name: Option<String>,
}

fn name_value(name: &str) -> Value {
    Value::obj(vec![("full", Value::str(name))])
}

fn roles_value(roles: &[String]) -> Value {
    Value::Arr(roles.iter().map(|r| Value::str(r)).collect())
}

fn clean_roles(roles: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in roles {
        let r = r.trim();
        if !r.is_empty() && !out.iter().any(|o| o == r) {
            out.push(r.to_string());
        }
    }
    out
}

/// Terms as stored: only the panel's terms, `true`/`false`; anything else
/// the record held is kept.
fn merged_terms(old: Option<&Value>, terms: &[(String, Option<bool>)]) -> Result<Value, String> {
    let mut pairs: Vec<(String, Value)> = match old {
        Some(Value::Obj(m)) => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => Vec::new(),
    };
    for (term, answer) in terms {
        if !CONSENT_TERMS.contains(&term.as_str()) {
            return Err(format!("unknown consent term {term}"));
        }
        pairs.retain(|(k, _)| k != term);
        if let Some(yes) = answer {
            pairs.push((term.clone(), Value::Bool(*yes)));
        }
    }
    Ok(Value::obj(pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect()))
}

impl Project {
    /// A new person with a name and roles (speaker, consultant, …).
    pub fn add_named_person(&mut self, name: &str, roles: &[String]) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("a person needs a name".into());
        }
        self.add_person(name_value(name), roles_value(&clean_roles(roles)), "open")
    }

    pub fn set_person_name(&mut self, person: &str, name: &str) -> Result<CommitResult, String> {
        self.require(person, "person")?;
        let name = name.trim();
        if name.is_empty() {
            return Err("a person needs a name".into());
        }
        let mut t = self.intent("edit-person").resolving();
        t.set("person", person, SCOPE_PEOPLE, "names", name_value(name))?;
        t.commit()
    }

    pub fn set_person_roles(&mut self, person: &str, roles: &[String]) -> Result<CommitResult, String> {
        self.require(person, "person")?;
        let mut t = self.intent("edit-person").resolving();
        t.set("person", person, SCOPE_PEOPLE, "roles", roles_value(&clean_roles(roles)))?;
        t.commit()
    }

    /// Delete a person and their consent records (tombstones; both come
    /// back with a restore, and the log keeps every value).
    pub fn delete_person(&mut self, person: &str) -> Result<CommitResult, String> {
        self.require(person, "person")?;
        let records = self.consents_of(person)?;
        let mut t = self.intent("delete-person");
        for c in &records {
            t.delete("consent", c, SCOPE_PEOPLE);
        }
        t.delete("person", person, SCOPE_PEOPLE);
        t.commit()
    }

    /// Record what `person` agreed to for `covers` (the project, a source or
    /// a text), with the panel's terms answered or left unasked.
    pub fn add_consent(&mut self, person: &str, covers: &str, terms: &[(String, Option<bool>)], date: &str) -> Result<String, String> {
        let terms = merged_terms(None, terms)?;
        self.record_consent(person, covers, terms, date.trim())
    }

    /// Answer, change or clear (`None`) some of a record's terms; the others
    /// stay as they are.
    pub fn set_consent_terms(&mut self, consent: &str, terms: &[(String, Option<bool>)]) -> Result<CommitResult, String> {
        self.require(consent, "consent")?;
        let old = self.proj.field(consent, "terms")?.map(|(v, _)| v);
        let value = merged_terms(old.as_ref(), terms)?;
        let mut t = self.intent("edit-consent").resolving();
        t.set("consent", consent, SCOPE_PEOPLE, "terms", value)?;
        t.commit()
    }

    pub fn set_consent_date(&mut self, consent: &str, date: &str) -> Result<CommitResult, String> {
        self.require(consent, "consent")?;
        let mut t = self.intent("edit-consent").resolving();
        t.set("consent", consent, SCOPE_PEOPLE, "date", Value::str(date.trim()))?;
        t.commit()
    }

    /// What the record covers: the project, a live source or a live text.
    pub fn set_consent_covers(&mut self, consent: &str, covers: &str) -> Result<CommitResult, String> {
        self.require(consent, "consent")?;
        if covers != self.project_id {
            match self.proj.entity_kind(covers)?.as_deref() {
                Some("source") => self.require(covers, "source")?,
                Some("text") => self.require(covers, "text")?,
                _ => return Err(format!("consent covers the project, a recording or a text, not {covers}")),
            }
        }
        let mut t = self.intent("edit-consent").resolving();
        t.set("consent", consent, SCOPE_PEOPLE, "covers", Value::str(covers))?;
        t.commit()
    }

    /// Attach the signed form (a blob already in the store), or detach it
    /// with `None`. The blob itself is never deleted: the log names it.
    pub fn set_consent_evidence(&mut self, consent: &str, evidence: Option<(&str, &str)>) -> Result<CommitResult, String> {
        self.require(consent, "consent")?;
        if let Some((blob, _)) = evidence {
            crate::check_blob_ref(blob)?;
        }
        let mut t = self.intent("edit-consent").resolving();
        let (blob, name) = match evidence {
            Some((b, n)) => (Value::str(b), Value::str(n)),
            None => (Value::Null, Value::Null),
        };
        t.set("consent", consent, SCOPE_PEOPLE, "evidence", blob)?;
        t.set("consent", consent, SCOPE_PEOPLE, "x.evidence-name", name)?;
        t.commit()
    }

    pub fn delete_consent(&mut self, consent: &str) -> Result<CommitResult, String> {
        self.require(consent, "consent")?;
        let mut t = self.intent("delete-consent");
        t.delete("consent", consent, SCOPE_PEOPLE);
        t.commit()
    }

    /// A person's live consent records, oldest first.
    fn consents_of(&self, person: &str) -> Result<Vec<String>, String> {
        let mut out = self.proj.children_by_ref("person", person, "consent")?;
        out.sort();
        Ok(out)
    }

    /// A string field, None when absent, null (cleared) or empty.
    fn text_field(&self, id: &str, field: &str) -> Result<Option<String>, String> {
        Ok(match self.proj.field(id, field)?.map(|(v, _)| v) {
            Some(Value::Str(s)) if !s.is_empty() => Some(s),
            _ => None,
        })
    }

    /// Everyone in the project with their consent records, by name.
    pub fn people(&self) -> Result<Vec<PersonRow>, String> {
        let mut rows = Vec::new();
        for id in self.proj.entities_of_kind("person")? {
            let names = self.proj.field(&id, "names")?.map(|(v, _)| v);
            let name = match &names {
                Some(Value::Obj(m)) => m
                    .get("full")
                    .and_then(|v| v.as_str())
                    .or_else(|| m.values().find_map(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string(),
                Some(Value::Str(s)) => s.clone(),
                _ => String::new(),
            };
            let roles = match self.proj.field(&id, "roles")?.map(|(v, _)| v) {
                Some(Value::Arr(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
                Some(Value::Str(s)) => vec![s],
                _ => Vec::new(),
            };
            let mut consents = Vec::new();
            for c in self.consents_of(&id)? {
                let terms = match self.proj.field(&c, "terms")?.map(|(v, _)| v) {
                    Some(Value::Obj(m)) => CONSENT_TERMS
                        .iter()
                        .filter_map(|t| match m.get(*t) {
                            Some(Value::Bool(b)) => Some((t.to_string(), *b)),
                            // Anything but `true` is a refusal (assist.rs reads it so).
                            Some(_) => Some((t.to_string(), false)),
                            None => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                consents.push(ConsentRow {
                    covers: self.proj.field_str(&c, "covers")?.unwrap_or_default(),
                    terms,
                    date: self.proj.field_str(&c, "date")?.unwrap_or_default(),
                    evidence: self.text_field(&c, "evidence")?,
                    evidence_name: self.text_field(&c, "x.evidence-name")?,
                    id: c,
                });
            }
            rows.push(PersonRow { id, name, roles, consents });
        }
        rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> (std::path::PathBuf, Project) {
        let root = std::env::temp_dir().join(format!("wugs-people-{}", wugs_model::ulid()));
        let p = Project::open(&root, "prs_Test").unwrap();
        (root, p)
    }

    fn t(term: &str, a: Option<bool>) -> (String, Option<bool>) {
        (term.to_string(), a)
    }

    #[test]
    fn a_person_and_their_consent_round_trip() {
        let (root, mut p) = project();
        let ana = p.add_named_person(" Ana Reyes ", &["speaker".into(), "speaker".into(), " ".into()]).unwrap();
        let pid = p.project_id.clone();
        let c = p.add_consent(&ana, &pid, &[t("archive-ok", Some(true)), t("AI-ok", Some(false)), t("web-ok", None)], "2026-11-11").unwrap();
        let people = p.people().unwrap();
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].name, "Ana Reyes");
        assert_eq!(people[0].roles, vec!["speaker".to_string()]);
        let rec = &people[0].consents[0];
        assert_eq!(rec.id, c);
        assert_eq!(rec.covers, pid);
        assert_eq!(rec.terms, vec![("archive-ok".to_string(), true), ("AI-ok".to_string(), false)], "web-ok was not asked");

        p.set_consent_terms(&c, &[t("AI-ok", Some(true)), t("archive-ok", None)]).unwrap();
        assert_eq!(p.people().unwrap()[0].consents[0].terms, vec![("AI-ok".to_string(), true)]);
        assert!(p.set_consent_terms(&c, &[t("sell-ok", Some(true))]).is_err());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn the_corrections_export_reads_what_the_panel_writes() {
        let (root, mut p) = project();
        let ana = p.add_named_person("Ana", &[]).unwrap();
        let pid = p.project_id.clone();
        let c = p.add_consent(&ana, &pid, &[t("AI-ok", Some(true))], "").unwrap();
        assert_eq!(p.ai_consent().unwrap().get(&pid), Some(&true));
        p.set_consent_terms(&c, &[t("AI-ok", Some(false))]).unwrap();
        assert_eq!(p.ai_consent().unwrap().get(&pid), Some(&false));
        p.set_consent_terms(&c, &[t("AI-ok", None)]).unwrap();
        assert_eq!(p.ai_consent().unwrap().get(&pid), None, "not asked says nothing");
        p.delete_person(&ana).unwrap();
        assert!(p.proj.is_tombstoned(&c).unwrap(), "their records go with them");
        assert!(p.people().unwrap().is_empty());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn covers_and_evidence_are_checked() {
        let (root, mut p) = project();
        let ana = p.add_named_person("Ana", &[]).unwrap();
        let pid = p.project_id.clone();
        let c = p.add_consent(&ana, &pid, &[], "").unwrap();
        let text = p.create_text("Story").unwrap();
        p.set_consent_covers(&c, &text).unwrap();
        assert!(p.set_consent_covers(&c, &ana).is_err(), "a person is not something consent covers");
        assert!(p.set_consent_evidence(&c, Some(("not-a-blob", "form.jpg"))).is_err());
        let blob = format!("blake3:{}", "a".repeat(64));
        p.set_consent_evidence(&c, Some((&blob, "form.jpg"))).unwrap();
        let rec = &p.people().unwrap()[0].consents[0];
        assert_eq!((rec.covers.as_str(), rec.evidence.as_deref(), rec.evidence_name.as_deref()), (text.as_str(), Some(blob.as_str()), Some("form.jpg")));
        p.set_consent_evidence(&c, None).unwrap();
        assert_eq!(p.people().unwrap()[0].consents[0].evidence, None);
        std::fs::remove_dir_all(root).ok();
    }
}
