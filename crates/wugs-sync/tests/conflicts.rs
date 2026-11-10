//! Conflict detection by write heads (DATA-MODEL §5), seen from devices
//! that edit offline and then sync. A conflict is an open version of a field
//! that no later write named; it does not matter who wrote it.

use std::fs;
use std::path::PathBuf;

use wugs_core::Project;
use wugs_sync::{sync_pair, SyncPolicy};

struct Rig {
    roots: Vec<PathBuf>,
}

impl Rig {
    fn new() -> Rig {
        Rig { roots: Vec::new() }
    }

    fn root(&mut self) -> PathBuf {
        let root = std::env::temp_dir().join(format!("wugs-conflicts-{}", wugs_model::ulid()));
        self.roots.push(root.clone());
        root
    }

    /// `actors.len()` devices of one project, the first one's actor first.
    fn devices(&mut self, actors: &[&str]) -> Vec<Project> {
        let first = Project::open(self.root(), actors[0]).unwrap();
        let id = first.project_id.clone();
        let mut out = vec![first];
        for a in &actors[1..] {
            out.push(Project::open_joined(self.root(), a, &id).unwrap());
        }
        out
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        for r in &self.roots {
            fs::remove_dir_all(r).ok();
        }
    }
}

fn sync(a: &mut Project, b: &mut Project) {
    let all = SyncPolicy::allow_all();
    sync_pair(a, b, &all, &all).unwrap();
}

fn sync_all(ps: &mut [Project]) {
    for _ in 0..2 {
        for i in 1..ps.len() {
            let (l, r) = ps.split_at_mut(i);
            sync(&mut l[0], &mut r[0]);
        }
    }
}

/// A morph every device holds, as (text, morph).
fn shared_morph(ps: &mut [Project]) -> (String, String) {
    let p = &mut ps[0];
    let txt = p.create_text("T").unwrap();
    let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "köpekler").unwrap().0;
    let tok = p.tokenize(&txt, &seg, &["köpekler"]).unwrap().0.remove(0);
    let mor = p.split_morph(&txt, &tok, &[("köpek", "root"), ("ler", "suffix")]).unwrap().0.remove(1);
    sync_all(ps);
    (txt, mor)
}

/// Conflict rows on the morph's gloss, as (winner value, loser value).
fn gloss_conflicts(p: &Project, mor: &str) -> Vec<(String, String)> {
    let value = |op: &str| {
        let ops = p.log.read_all().unwrap().ops;
        ops.into_iter().find(|o| o.id == op).unwrap().value.as_str().unwrap().to_string()
    };
    let mut s = p.proj.db.prepare("SELECT winner_op, loser_op FROM conflicts WHERE target=?1 AND field='gloss'").unwrap();
    s.bind_text(1, mor).unwrap();
    let mut rows = Vec::new();
    while s.step().unwrap() {
        rows.push((value(&s.col_text(0)), value(&s.col_text(1))));
    }
    rows.sort();
    rows
}

fn gloss(p: &Project, mor: &str) -> String {
    p.proj.field_str(mor, "gloss").unwrap().unwrap_or_default()
}

/// The values open against the shown gloss. Which concurrent write wins
/// depends on the clocks; that every other open one is offered does not.
fn open_against_winner(p: &Project, mor: &str) -> Vec<String> {
    let winner = gloss(p, mor);
    gloss_conflicts(p, mor)
        .into_iter()
        .map(|(w, l)| {
            assert_eq!(w, winner, "every row names the shown value");
            l
        })
        .collect()
}

fn without(all: &[&str], winner: &str) -> Vec<String> {
    let mut v: Vec<String> = all.iter().filter(|g| **g != winner).map(|g| g.to_string()).collect();
    v.sort();
    v
}

#[test]
fn a_follow_up_edit_on_one_device_keeps_the_other_devices_value_in_conflict() {
    // A sets PL; B, not having seen it, sets PLU and then PLURAL on top of
    // its own PLU. Nothing B wrote saw PL, so PL against PLURAL is open.
    let mut rig = Rig::new();
    let mut ps = rig.devices(&["prs_Ana", "prs_Bo"]);
    let (txt, mor) = shared_morph(&mut ps);
    ps[0].set_gloss(&txt, &mor, "PL", None).unwrap();
    ps[1].set_gloss(&txt, &mor, "PLU", None).unwrap();
    ps[1].set_gloss(&txt, &mor, "PLURAL", None).unwrap();
    sync_all(&mut ps);
    for p in &ps {
        let winner = gloss(p, &mor);
        assert_eq!(open_against_winner(p, &mor), without(&["PL", "PLURAL"], &winner), "PLU was seen by PLURAL; PL was not");
    }
}

#[test]
fn one_person_on_two_laptops_gets_the_conflict_prompt() {
    let mut rig = Rig::new();
    let mut ps = rig.devices(&["prs_Ana", "prs_Ana"]);
    let (txt, mor) = shared_morph(&mut ps);
    ps[0].set_gloss(&txt, &mor, "PL", None).unwrap();
    ps[1].set_gloss(&txt, &mor, "PLU", None).unwrap();
    sync_all(&mut ps);
    for p in &ps {
        let winner = gloss(p, &mor);
        assert_eq!(open_against_winner(p, &mor), without(&["PL", "PLU"], &winner));
    }
}

#[test]
fn three_devices_writing_at_once_leave_two_losers() {
    let mut rig = Rig::new();
    let mut ps = rig.devices(&["prs_Ana", "prs_Bo", "prs_Cem"]);
    let (txt, mor) = shared_morph(&mut ps);
    for (p, g) in ps.iter_mut().zip(["PL", "PLU", "PLURAL"]) {
        p.set_gloss(&txt, &mor, g, None).unwrap();
    }
    sync_all(&mut ps);
    for p in &ps {
        let winner = gloss(p, &mor);
        assert_eq!(open_against_winner(p, &mor), without(&["PL", "PLU", "PLURAL"], &winner));
        assert_eq!(gloss_conflicts(p, &mor), gloss_conflicts(&ps[0], &mor), "the same rows everywhere");
    }
}

#[test]
fn an_edit_made_after_the_sync_resolves_the_conflict_on_every_device() {
    let mut rig = Rig::new();
    let mut ps = rig.devices(&["prs_Ana", "prs_Bo"]);
    let (txt, mor) = shared_morph(&mut ps);
    ps[0].set_gloss(&txt, &mor, "PL", None).unwrap();
    ps[1].set_gloss(&txt, &mor, "PLU", None).unwrap();
    sync_all(&mut ps);
    assert_eq!(open_against_winner(&ps[0], &mor).len(), 1);

    // A has seen both, so its edit names both and closes the conflict,
    // even though it rewrites the value that lost.
    ps[0].set_gloss(&txt, &mor, "PL", None).unwrap();
    assert!(gloss_conflicts(&ps[0], &mor).is_empty(), "closed on the device that wrote");
    assert_eq!(gloss_conflicts(&ps[1], &mor).len(), 1, "still open where the edit has not arrived");
    sync_all(&mut ps);
    for p in &ps {
        assert!(gloss_conflicts(p, &mor).is_empty());
        assert_eq!(gloss(p, &mor), "PL");
        let kept = p.log.read_all().unwrap().ops.iter().any(|o| o.value.as_str() == Some("PLU"));
        assert!(kept, "the losing value stays in the log");
    }
}
