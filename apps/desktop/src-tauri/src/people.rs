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
    /// Deleted on another device while a record was added for them here.
    pub deleted: bool,
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
            deleted: r.deleted,
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

/// What the consent records say about one text (the Checks panel): records
/// inform and never block, so this only points out what is missing.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextConsent {
    pub text_id: String,
    pub title: String,
    /// Live records covering the project, the text or its recording.
    pub records: usize,
    /// Terms some covering record answers no (community-only aside).
    pub no: Vec<String>,
    /// Some covering record limits it to the community.
    pub community_only: bool,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConsentGaps {
    /// Texts with no covering record first, then texts a record limits, by title.
    pub texts: Vec<TextConsent>,
    /// Live texts.
    pub total: usize,
    /// Live consent records of live people.
    pub records: usize,
}

pub fn consent_gaps(s: &mut Session) -> Result<ConsentGaps, String> {
    let p = project(s)?;
    let people = p.people()?;
    let records: Vec<_> = people.iter().filter(|x| !x.deleted).flat_map(|x| x.consents.iter()).collect();
    let mut out = ConsentGaps { records: records.len(), ..ConsentGaps::default() };
    let mut texts = p.proj.entities_of_kind("text")?;
    texts.sort();
    for text in texts {
        out.total += 1;
        let source = p.proj.field_str(&text, "source")?.filter(|x| !x.is_empty());
        let covering: Vec<_> = records.iter().filter(|r| r.covers == p.project_id || r.covers == text || Some(&r.covers) == source.as_ref()).collect();
        let mut no: Vec<String> = covering.iter().flat_map(|r| r.terms.iter()).filter(|(t, b)| !b && t != "community-only").map(|(t, _)| t.clone()).collect();
        no.sort();
        no.dedup();
        let community_only = covering.iter().any(|r| r.terms.iter().any(|(t, b)| t == "community-only" && *b));
        if covering.is_empty() || !no.is_empty() || community_only {
            out.texts.push(TextConsent { title: p.proj.field_str(&text, "title")?.unwrap_or_default(), text_id: text, records: covering.len(), no, community_only });
        }
    }
    out.texts.sort_by_cached_key(|t| (t.records > 0, wugs_model::fold_key(&t.title), t.text_id.clone()));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_without_consent_or_with_limits_are_listed() {
        let dir = std::env::temp_dir().join(format!("wugs-consent-gaps-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let p = project(&mut s).unwrap();
        let a = p.create_text("Alpha").unwrap();
        let b = p.create_text("beta").unwrap();
        let c = p.create_text("Gamma").unwrap();
        let g = consent_gaps(&mut s).unwrap();
        assert_eq!((g.total, g.records, g.texts.len()), (3, 0, 3), "nothing recorded: every text listed");
        add_person(&mut s, "Ana", "speaker").unwrap();
        let ana = view(&mut s).unwrap().people[0].id.clone();
        add_consent(&mut s, &ana, &a, "2026-11-23").unwrap();
        let rec = view(&mut s).unwrap().people[0].consents[0].id.clone();
        set_term(&mut s, &rec, "web-ok", Some(false)).unwrap();
        add_consent(&mut s, &ana, &b, "2026-11-23").unwrap();
        let g = consent_gaps(&mut s).unwrap();
        assert_eq!(g.records, 2);
        let rows: Vec<_> = g.texts.iter().map(|t| (t.title.as_str(), t.records, t.no.clone())).collect();
        assert_eq!(rows, [("Gamma", 0, vec![]), ("Alpha", 1, vec!["web-ok".to_string()])], "beta is covered and unlimited; uncovered first");
        // A record for the whole project covers Gamma too.
        let p = project(&mut s).unwrap();
        let pid = p.project_id.clone();
        add_consent(&mut s, &ana, &pid, "2026-11-23").unwrap();
        assert!(consent_gaps(&mut s).unwrap().texts.iter().all(|t| t.text_id != c));
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
