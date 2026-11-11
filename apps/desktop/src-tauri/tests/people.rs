//! The People panel through the desktop session, and what it feeds: the
//! corrections export's consent column.

use std::fs;
use std::path::PathBuf;

use wugs_desktop::people;
use wugs_desktop::session::{self, Session};
use wugs_model::ulid;

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn a_speaker_their_consent_and_the_signed_form() {
    let dir = TempDir(std::env::temp_dir().join(format!("wugs-people-test-{}", ulid())));
    let mut s: Session = None;
    session::create_project(&mut s, dir.0.join("p").to_str().unwrap(), "prs_Test").unwrap();
    let text = session::create_text(&mut s, "Counting").unwrap();

    let v = people::add_person(&mut s, "Ana Reyes", "speaker").unwrap();
    let ana = v.people[0].id.clone();
    assert_eq!(v.coverable.iter().map(|c| c.kind).collect::<Vec<_>>(), vec!["project", "text"]);
    let project_id = v.coverable[0].id.clone();

    let v = people::add_consent(&mut s, &ana, &project_id, "2026-11-11").unwrap();
    let c = v.people[0].consents[0].id.clone();
    assert!(v.people[0].consents[0].terms.iter().all(|(_, a)| a.is_none()), "a new record asks nothing yet");

    people::set_term(&mut s, &c, "AI-ok", Some(true)).unwrap();
    people::set_term(&mut s, &c, "web-ok", Some(false)).unwrap();
    let v = people::set_covers(&mut s, &c, &text).unwrap();
    let rec = &v.people[0].consents[0];
    assert_eq!(rec.covers, text);
    assert_eq!(rec.terms.iter().find(|(t, _)| t == "AI-ok").unwrap().1, Some(true));
    assert_eq!(rec.terms.iter().find(|(t, _)| t == "web-ok").unwrap().1, Some(false));

    let scan = dir.0.join("form.jpg");
    fs::write(&scan, b"not a real photo").unwrap();
    let v = people::attach(&mut s, &c, scan.to_str().unwrap()).unwrap();
    assert!(v.people[0].consents[0].has_evidence);
    assert_eq!(v.people[0].consents[0].evidence_name.as_deref(), Some("form.jpg"));
    let p = s.as_ref().unwrap();
    assert!(p.blob_refs().unwrap().iter().any(|r| r.starts_with("blake3:")), "the form is kept and GC sees it");

    let v = people::delete_person(&mut s, &ana).unwrap();
    assert!(v.people.is_empty());
}
