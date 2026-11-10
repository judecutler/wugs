use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};
use wugs_blob::BlobStore;
use wugs_core::Project;

const GRACE: Duration = Duration::from_secs(3600);

fn backdate(path: &Path) {
    let f = fs::File::options().write(true).open(path).unwrap();
    f.set_modified(SystemTime::now() - GRACE * 2).unwrap();
}

#[test]
fn gc_keeps_referenced_sweeps_old_orphans_and_spares_young_ones() {
    let root = std::env::temp_dir().join(format!("wugs-blob-gc-{}", wugs_model::ulid()));
    let mut p = Project::open(&root, "prs_Alice").unwrap();
    let store = BlobStore::open(&root).unwrap();

    let a = store.put_bytes(b"recording of speaker one").unwrap();
    let b = store.put_bytes(b"orphan: abandoned import").unwrap();
    let c = store.put_bytes(b"orphan: just imported").unwrap();
    let src = p.add_source("Elicitation 1", None, "audio", None).unwrap();
    p.add_media_track(&src, &a, "audio", "main", 0).unwrap();
    backdate(&store.path_of(&a).unwrap());
    backdate(&store.path_of(&b).unwrap());

    let rep = store.gc(p.blob_refs().unwrap(), GRACE).unwrap();
    assert_eq!((rep.kept, rep.swept, rep.spared_young), (1, 1, 1), "{rep:?}");
    assert_eq!(rep.swept_bytes, b"orphan: abandoned import".len() as u64);
    assert!(store.has(&a));
    assert!(!store.has(&b));
    assert!(store.has(&c));
    assert!(store.verify(&a).unwrap());
    fs::remove_dir_all(root).ok();
}

#[test]
fn tombstoned_track_keeps_its_blob_through_gc() {
    let root = std::env::temp_dir().join(format!("wugs-blob-gc-{}", wugs_model::ulid()));
    let mut p = Project::open(&root, "prs_Alice").unwrap();
    let store = BlobStore::open(&root).unwrap();

    let a = store.put_bytes(b"deleted but restorable").unwrap();
    let src = p.add_source("S", None, "video", None).unwrap();
    let trk = p.add_media_track(&src, &a, "video", "cam", 0).unwrap();
    let mut t = p.intent("delete-track");
    t.delete("media-track", &trk, &src);
    t.commit().unwrap();
    assert!(p.proj.is_tombstoned(&trk).unwrap());
    backdate(&store.path_of(&a).unwrap());

    let rep = store.gc(p.blob_refs().unwrap(), GRACE).unwrap();
    assert_eq!((rep.kept, rep.swept), (1, 0), "{rep:?}");
    assert!(store.has(&a));
    fs::remove_dir_all(root).ok();
}

/// Every `.jsonl` log file under `from`, copied raw into `to` (another
/// device's ops arriving on disk without passing through the projector).
fn copy_logs(from: &Path, to: &Path) {
    for scope in fs::read_dir(from.join("log")).unwrap() {
        let scope = scope.unwrap().path();
        let dest = to.join("log").join(scope.file_name().unwrap());
        fs::create_dir_all(&dest).unwrap();
        for file in fs::read_dir(&scope).unwrap() {
            let file = file.unwrap().path();
            fs::copy(&file, dest.join(file.file_name().unwrap())).unwrap();
        }
    }
}

#[test]
fn gc_spares_blobs_the_log_references_but_the_projection_has_not_seen() {
    let root = std::env::temp_dir().join(format!("wugs-blob-gc-{}", wugs_model::ulid()));
    let other = std::env::temp_dir().join(format!("wugs-blob-gc-{}", wugs_model::ulid()));
    let p = Project::open(&root, "prs_Alice").unwrap();
    let store = BlobStore::open(&root).unwrap();
    let x = store.put_bytes(b"recording the projection never saw").unwrap();
    let torn = store.put_bytes(b"recording whose op line was torn").unwrap();

    let mut bob = Project::open(&other, "prs_Bob").unwrap();
    let src = bob.add_source("S", None, "audio", None).unwrap();
    bob.add_media_track(&src, &x, "audio", "main", 0).unwrap();
    copy_logs(&other, &root);
    // a crash mid-append: the line stops right after the ref
    let tail = root.join("log").join(&src).join("crashed-device.jsonl");
    fs::write(&tail, format!("{{\"verb\":\"set\",\"field\":\"blob\",\"value\":\"{torn}")).unwrap();
    assert!(p.proj.field_str(&src, "label").unwrap().is_none(), "projection has not replayed the copied log");

    let refs = p.blob_refs().unwrap();
    assert!(refs.contains(&x) && refs.contains(&torn), "{refs:?}");
    backdate(&store.path_of(&x).unwrap());
    backdate(&store.path_of(&torn).unwrap());
    let rep = store.gc(refs, GRACE).unwrap();
    assert_eq!((rep.kept, rep.swept), (2, 0), "{rep:?}");
    assert!(store.verify(&x).unwrap() && store.verify(&torn).unwrap());
    fs::remove_dir_all(root).ok();
    fs::remove_dir_all(other).ok();
}
