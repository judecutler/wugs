use std::fs;
use std::path::{Path, PathBuf};

use wugs_core::{Engines, Project};
use wugs_model::{ulid, Op};
use wugs_text::LoroMerger;

fn open(root: &Path, actor: &str) -> Project {
    Project::open_with(root, actor, Engines { merger: Some(Box::new(LoroMerger)), ..Default::default() }).unwrap()
}

fn own_ops(p: &Project) -> Vec<Op> {
    let me = p.device().to_string();
    p.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == me).collect()
}

fn content(p: &Project, seg: &str) -> String {
    p.proj.field_str(seg, "content").unwrap().unwrap()
}

/// The cache as it stood when the project was last closed.
struct CacheSnapshot(PathBuf);

impl CacheSnapshot {
    fn take(root: &Path) -> CacheSnapshot {
        let to = root.with_extension("snapshot.sqlite");
        fs::copy(root.join("cache/projection.sqlite"), &to).unwrap();
        CacheSnapshot(to)
    }

    /// What a crash after the log append and before the savepoint release
    /// leaves: every op since the snapshot is in the log, none in the cache.
    fn restore(self, root: &Path) {
        for side in ["projection.sqlite-wal", "projection.sqlite-shm"] {
            fs::remove_file(root.join("cache").join(side)).ok();
        }
        fs::rename(&self.0, root.join("cache/projection.sqlite")).unwrap();
    }
}

#[test]
fn a_caught_up_text_edit_keeps_its_lineage_for_later_merges() {
    let root = std::env::temp_dir().join(format!("wugs-text-catchup-{}", ulid()));
    let bob_root = std::env::temp_dir().join(format!("wugs-text-catchup-{}", ulid()));
    let mut alice = open(&root, "prs_Alice");
    let txt = alice.create_text("Shared").unwrap();
    let seg = alice.add_segment(&txt, "tir_utt", 0, None, None, "köpek geldi").unwrap().0;
    alice.edit_content(&txt, &seg, "köpek geldi dün").unwrap();
    let mut bob = open(&bob_root, "prs_Bob");
    bob.ingest(own_ops(&alice)).unwrap();
    drop(alice);

    let snapshot = CacheSnapshot::take(&root);
    let mut alice = open(&root, "prs_Alice");
    alice.edit_content(&txt, &seg, "büyük köpek geldi dün").unwrap();
    drop(alice);
    snapshot.restore(&root);

    let mut alice = open(&root, "prs_Alice");
    assert_eq!(alice.caught_up(), 1, "the edit the crash hid");
    assert_eq!(content(&alice, &seg), "büyük köpek geldi dün");

    alice.edit_content(&txt, &seg, "büyük köpek geldi").unwrap();
    assert_eq!(content(&alice, &seg), "büyük köpek geldi", "a local edit after the catch-up diffs against the caught-up text");
    bob.edit_content(&txt, &seg, "köpek geldi dün akşam").unwrap();
    bob.ingest(own_ops(&alice)).unwrap();
    alice.ingest(own_ops(&bob)).unwrap();
    let merged = content(&alice, &seg);
    assert_eq!(content(&bob, &seg), merged, "both devices converge");
    assert_eq!(merged.matches("köpek geldi").count(), 1, "no text doubled: {merged}");
    assert!(merged.starts_with("büyük "), "alice's caught-up insert survives the merge: {merged}");
    assert_eq!(alice.proj.count("conflicts").unwrap(), 0);
    drop(alice);

    fs::remove_dir_all(root.join("cache")).unwrap();
    let alice = open(&root, "prs_Alice");
    assert_eq!(content(&alice, &seg), merged, "a rebuild from the log agrees with the caught-up cache");
    drop(alice);
    drop(bob);
    fs::remove_dir_all(&root).ok();
    fs::remove_dir_all(&bob_root).ok();
}
