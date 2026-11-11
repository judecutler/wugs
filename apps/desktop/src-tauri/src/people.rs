//! The People panel's commands (roadmap 61): who is in the project and what
//! each agreed to. Every command returns the whole view, which is small.

use std::path::Path;

use serde::Serialize;
use wugs_blob::BlobStore;
use wugs_core::people::CONSENT_TERMS;

use crate::session::{project, Session};

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConsentView {
    pub id: String,
    pub covers: String,
    /// One per panel term, in order: true agreed, false refused, null not asked.
    pub terms: Vec<(String, Option<bool>)>,
    pub date: String,
    pub evidence_name: Option<String>,
    pub has_evidence: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PersonView {
    pub id: String,
    pub name: String,
    pub roles: Vec<String>,
    pub consents: Vec<ConsentView>,
}

/// Something a consent record can cover.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Coverable {
    pub id: String,
    /// "project", "recording" or "text".
    pub kind: &'static str,
    pub label: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeopleView {
    pub people: Vec<PersonView>,
    pub coverable: Vec<Coverable>,
}

pub fn view(s: &mut Session) -> Result<PeopleView, String> {
    let p = project(s)?;
    let people = p
        .people()?
        .into_iter()
        .map(|r| PersonView {
            id: r.id,
            name: r.name,
            roles: r.roles,
            consents: r
                .consents
                .into_iter()
                .map(|c| ConsentView {
                    terms: CONSENT_TERMS
                        .iter()
                        .map(|t| (t.to_string(), c.terms.iter().find(|(k, _)| k == t).map(|(_, v)| *v)))
                        .collect(),
                    id: c.id,
                    covers: c.covers,
                    date: c.date,
                    has_evidence: c.evidence.is_some(),
                    evidence_name: c.evidence_name,
                })
                .collect(),
        })
        .collect();
    let mut coverable = vec![Coverable { id: p.project_id.clone(), kind: "project", label: "The whole project".into() }];
    let mut add = |kind: &'static str, entity: &str, field: &str| -> Result<(), String> {
        let mut rows = Vec::new();
        for id in p.proj.entities_of_kind(entity)? {
            let label = p.proj.field_str(&id, field)?.unwrap_or_default();
            rows.push(Coverable { label: if label.is_empty() { "(untitled)".into() } else { label }, id, kind });
        }
        rows.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
        coverable.extend(rows);
        Ok(())
    };
    add("recording", "source", "label")?;
    add("text", "text", "title")?;
    Ok(PeopleView { people, coverable })
}

pub fn add_person(s: &mut Session, name: &str, role: &str) -> Result<PeopleView, String> {
    let roles: Vec<String> = if role.trim().is_empty() { Vec::new() } else { vec![role.trim().to_string()] };
    project(s)?.add_named_person(name, &roles)?;
    view(s)
}

pub fn set_name(s: &mut Session, person: &str, name: &str) -> Result<PeopleView, String> {
    project(s)?.set_person_name(person, name)?;
    view(s)
}

pub fn set_roles(s: &mut Session, person: &str, roles: &[String]) -> Result<PeopleView, String> {
    project(s)?.set_person_roles(person, roles)?;
    view(s)
}

pub fn delete_person(s: &mut Session, person: &str) -> Result<PeopleView, String> {
    project(s)?.delete_person(person)?;
    view(s)
}

pub fn add_consent(s: &mut Session, person: &str, covers: &str, date: &str) -> Result<PeopleView, String> {
    project(s)?.add_consent(person, covers, &[], date)?;
    view(s)
}

pub fn set_term(s: &mut Session, consent: &str, term: &str, answer: Option<bool>) -> Result<PeopleView, String> {
    project(s)?.set_consent_terms(consent, &[(term.to_string(), answer)])?;
    view(s)
}

pub fn set_covers(s: &mut Session, consent: &str, covers: &str) -> Result<PeopleView, String> {
    project(s)?.set_consent_covers(consent, covers)?;
    view(s)
}

pub fn set_date(s: &mut Session, consent: &str, date: &str) -> Result<PeopleView, String> {
    project(s)?.set_consent_date(consent, date)?;
    view(s)
}

/// Copy a scan or photo of the signed form into the blob store and attach
/// it. The file goes into the project like a recording (and into sealed
/// packages); the people scope's own access is what restricts it.
pub fn attach(s: &mut Session, consent: &str, path: &str) -> Result<PeopleView, String> {
    let file = Path::new(path);
    let name = file.file_name().and_then(|n| n.to_str()).ok_or_else(|| format!("{path} has no usable file name"))?.to_string();
    let p = project(s)?;
    let store = BlobStore::open(&p.root).map_err(|e| format!("open blob store: {e}"))?;
    let blob = store.put_file(file).map_err(|e| format!("store {name}: {e}"))?;
    p.set_consent_evidence(consent, Some((&blob, &name)))?;
    view(s)
}

pub fn detach(s: &mut Session, consent: &str) -> Result<PeopleView, String> {
    project(s)?.set_consent_evidence(consent, None)?;
    view(s)
}

pub fn delete_consent(s: &mut Session, consent: &str) -> Result<PeopleView, String> {
    project(s)?.delete_consent(consent)?;
    view(s)
}
