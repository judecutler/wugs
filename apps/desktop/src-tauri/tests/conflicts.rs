use std::fs;
use std::path::PathBuf;

use wugs_core::Project;
use wugs_desktop::conflicts;
use wugs_desktop::session::{self, Session};
use wugs_model::ulid;

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn mine(p: &Project) -> Vec<wugs_model::Op> {
    p.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == p.device()).collect()
}

/// Ana and Bo gloss one sense differently and both edit one transcription;
/// Ana's folder ends up holding both sides. Returns (Ana's folder, Bo's, sense, segment).
fn conflicted() -> (TempDir, TempDir, String, String) {
    let (da, db) = (
        TempDir(std::env::temp_dir().join(format!("wugs-desktop-conflicts-a-{}", ulid()))),
        TempDir(std::env::temp_dir().join(format!("wugs-desktop-conflicts-b-{}", ulid()))),
    );
    let mut a = Project::open(&da.0, "prs_Ana").unwrap();
    let mut b = Project::open(&db.0, "prs_Bo").unwrap();
    let ((_, sen), _) = a.add_lexeme("ev", "house").unwrap();
    let txt = a.create_text("Stories").unwrap();
    let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "bir ev").unwrap().0;
    b.ingest(a.log.read_all().unwrap().ops).unwrap();
    a.set_sense_gloss(&sen, "home", None).unwrap();
    b.set_sense_gloss(&sen, "dwelling", None).unwrap();
    a.edit_content(&txt, &seg, "bir ev var").unwrap();
    b.edit_content(&txt, &seg, "bir evde").unwrap();
    a.ingest(mine(&b)).unwrap();
    (da, db, sen, seg)
}

#[test]
fn the_queue_describes_each_conflict_and_a_pick_settles_it() {
    let (da, _db, sen, seg) = conflicted();
    let mut s: Session = None;
    session::open_project(&mut s, da.0.to_str().unwrap(), "prs_Ana").unwrap();
    let list = conflicts::conflicts_list(&mut s).unwrap();
    assert_eq!(list.len(), 2, "{list:?}");
    let gloss = list.iter().find(|c| c.target == sen).unwrap();
    assert!(gloss.title.starts_with("Gloss of «ev»"), "{}", gloss.title);
    assert_eq!(gloss.place, "Lexicon");
    assert!(gloss.editable);
    let mut labels: Vec<&str> = gloss.versions.iter().map(|v| v.label.as_str()).collect();
    labels.sort();
    assert_eq!(labels, ["dwelling", "home"]);
    assert_eq!(gloss.versions.iter().filter(|v| v.winner).count(), 1);
    assert_eq!(gloss.versions.iter().filter(|v| v.mine).count(), 1);

    let content = list.iter().find(|c| c.target == seg).unwrap();
    assert_eq!(content.place, "Text «Stories»");
    assert_eq!(content.segment_id.as_deref(), Some(seg.as_str()));
    assert!(content.title.starts_with("Transcription of"));

    // Pick Bo's gloss; type a third transcription.
    let bo = gloss.versions.iter().find(|v| !v.mine).unwrap().raw.clone();
    // A choice made on a list that no longer matches is refused, with the fresh list.
    let stale = conflicts::conflict_resolve(&mut s, &sen, "gloss", &bo, &gloss.heads[..1]).unwrap();
    assert!(stale.notes[0].contains("changed again since"), "{:?}", stale.notes);
    assert_eq!(stale.conflicts.len(), 2, "nothing written");
    let after = conflicts::conflict_resolve(&mut s, &sen, "gloss", &bo, &gloss.heads).unwrap();
    assert_eq!(after.conflicts.len(), 1);
    let content_heads = content.heads.clone();
    let after = conflicts::conflict_resolve(&mut s, &seg, "content", &wugs_model::Value::str("bir ev vardı").to_json(), &content_heads).unwrap();
    assert!(after.conflicts.is_empty());
    assert_eq!(session::conflict_count(&mut s).unwrap(), 0);
}

#[test]
fn a_reference_is_shown_by_what_it_names_and_is_not_typed() {
    let (da, db) = (
        TempDir(std::env::temp_dir().join(format!("wugs-desktop-conflicts-a-{}", ulid()))),
        TempDir(std::env::temp_dir().join(format!("wugs-desktop-conflicts-b-{}", ulid()))),
    );
    let mut a = Project::open(&da.0, "prs_Ana").unwrap();
    let mut b = Project::open(&db.0, "prs_Bo").unwrap();
    let ((_, dog), _) = a.add_lexeme("köpek", "dog").unwrap();
    let ((_, hound), _) = a.add_lexeme("köpek", "hound").unwrap();
    let txt = a.create_text("T").unwrap();
    let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "köpek").unwrap().0;
    let tok = a.tokenize(&txt, &seg, &["köpek"]).unwrap().0[0].clone();
    let mor = a.split_morph(&txt, &tok, &[("köpek", "root")]).unwrap().0[0].clone();
    b.ingest(a.log.read_all().unwrap().ops).unwrap();
    a.link_sense(&txt, &mor, &dog).unwrap();
    b.link_sense(&txt, &mor, &hound).unwrap();
    a.ingest(mine(&b)).unwrap();
    drop(a);
    let mut s: Session = None;
    session::open_project(&mut s, da.0.to_str().unwrap(), "prs_Ana").unwrap();
    let list = conflicts::conflicts_list(&mut s).unwrap();
    let c = list.iter().find(|c| c.field == "sense").unwrap();
    assert!(!c.editable);
    let mut labels: Vec<&str> = c.versions.iter().map(|v| v.label.as_str()).collect();
    labels.sort();
    assert_eq!(labels, ["«köpek» ‘dog’", "«köpek» ‘hound’"]);
    assert_eq!(c.title, "Sense link of «köpek»");
    assert_eq!(c.place, "Text «T»");
}

#[test]
fn two_pins_read_as_the_project_model_with_model_names() {
    let (da, db) = (
        TempDir(std::env::temp_dir().join(format!("wugs-desktop-pins-a-{}", ulid()))),
        TempDir(std::env::temp_dir().join(format!("wugs-desktop-pins-b-{}", ulid()))),
    );
    let mut a = Project::open(&da.0, "prs_Ana").unwrap();
    let mut b = Project::open(&db.0, "prs_Bo").unwrap();
    a.set_setting("capability:phone-transcription", "wav2vec2-espeak").unwrap();
    b.set_setting("capability:phone-transcription", "zipa-small").unwrap();
    a.ingest(mine(&b)).unwrap();
    drop(a);
    let mut s: Session = None;
    session::open_project(&mut s, da.0.to_str().unwrap(), "prs_Ana").unwrap();
    let list = conflicts::conflicts_list(&mut s).unwrap();
    assert_eq!(list.len(), 1, "{list:?}");
    assert_eq!(list[0].title, "This project's model for phone transcription");
    assert_eq!(list[0].place, "Project settings");
    let mut labels: Vec<&str> = list[0].versions.iter().map(|v| v.label.as_str()).collect();
    labels.sort();
    assert_eq!(labels, ["wav2vec 2.0 phones (espeak IPA)", "zipa-small"]);
}
