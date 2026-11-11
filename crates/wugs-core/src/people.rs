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
    /// The person was deleted (on another device, while this one added a
    /// record for them): their live records are still listed, so they can
    /// be seen and deleted, because the corrections export still reads them.
    pub deleted: bool,
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
        // Names in other keys (language-tagged, from an importer) are kept.
        let value = match self.proj.field(person, "names")?.map(|(v, _)| v) {
            Some(Value::Obj(mut m)) => {
                m.insert("full".into(), Value::str(name));
                Value::Obj(m)
            }
            _ => name_value(name),
        };
        let mut t = self.intent("edit-person").resolving();
        t.set("person", person, SCOPE_PEOPLE, "names", value)?;
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
    /// stay as they are. A plain write, not a resolving one: all terms share
    /// one field, so a concurrent answer to another term from another
    /// device must stay an open conflict for a person to look at, never be
    /// closed unseen by an edit of a different term.
    pub fn set_consent_terms(&mut self, consent: &str, terms: &[(String, Option<bool>)]) -> Result<CommitResult, String> {
        self.require(consent, "consent")?;
        let old = self.proj.field(consent, "terms")?.map(|(v, _)| v);
        let value = merged_terms(old.as_ref(), terms)?;
        let mut t = self.intent("edit-consent");
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

    /// Everyone in the project with their consent records, by name, then
    /// any live records whose person was deleted.
    pub fn people(&self) -> Result<Vec<PersonRow>, String> {
        let mut rows = Vec::new();
        let live: Vec<String> = self.proj.entities_of_kind("person")?;
        let mut orphans: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for c in self.proj.entities_of_kind("consent")? {
            let person = self.proj.field_str(&c, "person")?.unwrap_or_default();
            if !live.contains(&person) {
                orphans.entry(person).or_default().push(c);
            }
        }
        for id in live.iter().cloned() {
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
            let consents = self.consent_rows(self.consents_of(&id)?)?;
            rows.push(PersonRow { id, name, roles, consents, deleted: false });
        }
        rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
        for (person, mut records) in orphans {
            records.sort();
            rows.push(PersonRow { id: person, name: String::new(), roles: Vec::new(), consents: self.consent_rows(records)?, deleted: true });
        }
        Ok(rows)
    }

    fn consent_rows(&self, ids: Vec<String>) -> Result<Vec<ConsentRow>, String> {
        let mut consents = Vec::new();
        {
            for c in ids {
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
        }
        Ok(consents)
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
    fn a_record_whose_person_was_deleted_stays_visible() {
        let (root, mut p) = project();
        let ana = p.add_named_person("Ana", &[]).unwrap();
        let pid = p.project_id.clone();
        // Another device deleted Ana while this one recorded consent: only the person goes.
        let mut t = p.intent("delete-person");
        t.delete("person", &ana, SCOPE_PEOPLE);
        t.commit().unwrap();
        let c = p.add_consent_unchecked_for_test(&ana, &pid);
        let rows = p.people().unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].deleted);
        assert_eq!(rows[0].consents[0].id, c);
        p.delete_consent(&c).unwrap();
        assert!(p.people().unwrap().is_empty());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn renaming_keeps_names_in_other_keys() {
        let (root, mut p) = project();
        let ana = p.add_person(Value::obj(vec![("en", Value::str("Ayşe"))]), Value::Arr(vec![]), "open").unwrap();
        p.set_person_name(&ana, "Ayşe Yılmaz").unwrap();
        let names = p.proj.field(&ana, "names").unwrap().unwrap().0;
        assert_eq!(names.get_str("en"), Some("Ayşe"));
        assert_eq!(names.get_str("full"), Some("Ayşe Yılmaz"));
        std::fs::remove_dir_all(root).ok();
    }

    impl Project {
        /// A record for a person this device no longer sees, as sync can deliver.
        fn add_consent_unchecked_for_test(&mut self, person: &str, covers: &str) -> String {
            let mut t = self.intent("record-consent");
            let id = t.create("consent", "cns", SCOPE_PEOPLE, vec![("person", Value::str(person)), ("covers", Value::str(covers)), ("terms", Value::obj(vec![("AI-ok", Value::Bool(false))]))]);
            t.commit().unwrap();
            id
        }
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
