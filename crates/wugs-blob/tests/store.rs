use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use wugs_blob::{parse_ref, BlobStore};

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("wugs-blob-test-{}", wugs_model::ulid()));
    fs::create_dir_all(&root).unwrap();
    root
}

const EMPTY_HEX: &str = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";

#[test]
fn blake3_vectors_name_the_blobs() {
    let root = temp_root();
    let store = BlobStore::open(&root).unwrap();
    assert_eq!(store.put_bytes(b"").unwrap(), format!("blake3:{EMPTY_HEX}"));
    let abc = store.put_bytes(b"abc").unwrap();
    assert_eq!(abc, "blake3:6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85");
    assert_eq!(parse_ref(&abc).unwrap().len(), 64);
    fs::remove_dir_all(root).ok();
}

#[test]
fn parse_ref_accepts_only_the_exact_format() {
    let ok = format!("blake3:{EMPTY_HEX}");
    assert_eq!(parse_ref(&ok), Some(EMPTY_HEX));
    for bad in [
        "",
        "blake3:",
        "blake3:abc",
        &format!("sha256:{EMPTY_HEX}"),
        &format!("blake3:{}", EMPTY_HEX.to_uppercase()),
        &format!("{ok}0"),
        &format!("blake3:{}g", &EMPTY_HEX[..63]),
        EMPTY_HEX,
    ] {
        assert_eq!(parse_ref(bad), None, "{bad:?}");
    }
}

#[test]
fn put_is_idempotent_and_never_replaces_the_file() {
    let root = temp_root();
    let store = BlobStore::open(&root).unwrap();
    let a = store.put_bytes(b"field recording").unwrap();
    let path = store.path_of(&a).unwrap();
    // A hard link survives only while the original file is untouched; a
    // replacement by rename leaves the stored blob with a link count of 1.
    let probe = root.join("probe-link");
    fs::hard_link(&path, &probe).unwrap();

    let src = root.join("copy.wav");
    fs::write(&src, b"field recording").unwrap();
    let b = store.put_file(&src).unwrap();
    let c = store.put_bytes(b"field recording").unwrap();
    assert_eq!((&a, &a), (&b, &c));

    #[cfg(unix)]
    assert_eq!(
        std::os::unix::fs::MetadataExt::nlink(&fs::metadata(&path).unwrap()),
        2,
        "existing blob file was replaced"
    );
    let names: Vec<String> = fs::read_dir(root.join("blobs"))
        .unwrap()
        .map(|e| e.unwrap())
        .inspect(|e| assert!(e.file_type().unwrap().is_file(), "{:?} is not a file", e.path()))
        .map(|e| e.file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, vec![parse_ref(&a).unwrap().to_string()], "blobs/ holds only the blob");
    assert_eq!(fs::read_dir(root.join("cache/blob-tmp")).unwrap().count(), 0, "no temp left behind");
    fs::remove_dir_all(root).ok();
}

#[test]
fn put_file_streams_a_multi_buffer_file() {
    let root = temp_root();
    let store = BlobStore::open(&root).unwrap();
    let big: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    let src = root.join("big.bin");
    fs::write(&src, &big).unwrap();
    let r = store.put_file(&src).unwrap();
    assert_eq!(r, format!("blake3:{}", blake3::hash(&big).to_hex()));
    assert_eq!(fs::read(store.path_of(&r).unwrap()).unwrap(), big);
    assert!(store.put_file(&root.join("missing")).is_err());
    fs::remove_dir_all(root).ok();
}

#[test]
fn verify_detects_corruption_and_path_of_rejects_bad_refs() {
    let root = temp_root();
    let store = BlobStore::open(&root).unwrap();
    let r = store.put_bytes(b"consent scan").unwrap();
    assert!(store.has(&r));
    assert!(store.verify(&r).unwrap());
    fs::write(store.path_of(&r).unwrap(), b"consent scaN").unwrap();
    assert!(!store.verify(&r).unwrap());

    assert_eq!(store.path_of("blake3:nope"), None);
    assert_eq!(store.path_of(&format!("blake3:{}", "0".repeat(64))), None, "valid but missing");
    assert!(!store.has("../etc/passwd"));
    assert!(!store.verify("blake3:nope").unwrap());
    fs::remove_dir_all(root).ok();
}

#[test]
fn leftover_tmp_file_does_not_break_open_or_put_and_gc_clears_it_after_grace() {
    let root = temp_root();
    fs::create_dir_all(root.join("cache/blob-tmp")).unwrap();
    let stale = root.join("cache/blob-tmp/crashed-upload");
    fs::write(&stale, b"half a file").unwrap();
    let store = BlobStore::open(&root).unwrap();
    let r = store.put_bytes(b"after the crash").unwrap();
    assert!(store.verify(&r).unwrap());

    let young = store.gc([r.clone()], Duration::from_secs(3600)).unwrap();
    assert_eq!(young.tmp_cleared, 0);
    assert!(stale.exists(), "young tmp entry survives");

    backdate(&stale, Duration::from_secs(7200));
    let rep = store.gc([r.clone()], Duration::from_secs(3600)).unwrap();
    assert_eq!(rep.tmp_cleared, 1);
    assert!(!stale.exists());
    assert!(store.has(&r));
    fs::remove_dir_all(root).ok();
}

#[test]
fn gc_ignores_non_blob_entries() {
    let root = temp_root();
    let store = BlobStore::open(&root).unwrap();
    fs::write(root.join("blobs/notes.txt"), b"x").unwrap();
    fs::create_dir(root.join("blobs/subdir")).unwrap();
    let rep = store.gc([], Duration::ZERO).unwrap();
    assert_eq!(rep.ignored, 2);
    assert_eq!(rep.swept, 0);
    assert!(root.join("blobs/notes.txt").exists());
    fs::remove_dir_all(root).ok();
}

#[test]
fn shared_vectors_agree_with_parse_ref() {
    for &(text, ok) in wugs_model::REF_VECTORS {
        assert_eq!(parse_ref(text).is_some(), ok, "{text:?}");
    }
}

#[test]
fn a_directory_squatting_on_a_blob_name_is_never_reported_as_the_blob() {
    let root = temp_root();
    let store = BlobStore::open(&root).unwrap();
    let r = format!("blake3:{}", blake3::hash(b"squatted").to_hex());
    let squat = root.join("blobs").join(parse_ref(&r).unwrap());
    fs::create_dir(&squat).unwrap();

    assert!(store.put_bytes(b"squatted").is_err(), "put must not return a ref with no file behind it");
    assert!(!store.has(&r));
    assert!(!store.verify(&r).unwrap());
    let rep = store.gc([], Duration::ZERO).unwrap();
    assert_eq!((rep.ignored, rep.swept), (1, 0), "{rep:?}");
    assert!(squat.is_dir());
    assert_eq!(fs::read_dir(root.join("cache/blob-tmp")).unwrap().count(), 0, "failed put cleans its temp");
    fs::remove_dir_all(root).ok();
}

#[test]
fn re_putting_an_old_blob_makes_it_young_again() {
    let root = temp_root();
    let store = BlobStore::open(&root).unwrap();
    let grace = Duration::from_secs(3600);
    let r = store.put_bytes(b"about to be referenced again").unwrap();
    backdate(&store.path_of(&r).unwrap(), grace * 2);

    assert_eq!(store.put_bytes(b"about to be referenced again").unwrap(), r);
    let rep = store.gc([], grace).unwrap();
    assert_eq!((rep.swept, rep.spared_young), (0, 1), "{rep:?}");
    assert!(store.verify(&r).unwrap());
    fs::remove_dir_all(root).ok();
}

#[test]
fn re_put_restores_a_blob_whose_file_vanished() {
    let root = temp_root();
    let store = BlobStore::open(&root).unwrap();
    let r = store.put_bytes(b"swept between two puts").unwrap();
    fs::remove_file(store.path_of(&r).unwrap()).unwrap();
    assert_eq!(store.put_bytes(b"swept between two puts").unwrap(), r);
    assert!(store.verify(&r).unwrap());
    fs::remove_dir_all(root).ok();
}

fn backdate(path: &std::path::Path, by: Duration) {
    let f = fs::File::options().write(true).open(path).unwrap();
    f.set_modified(std::time::SystemTime::now() - by).unwrap();
}
