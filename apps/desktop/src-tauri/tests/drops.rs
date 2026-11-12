//! Files dropped onto the window: copied into the project's cache in pieces,
//! then imported like a picked file under the name they were dropped with.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use wugs_desktop::drops::{self, Drops, Route};
use wugs_desktop::interchange::{self, Format};
use wugs_desktop::{capture, convert};
use wugs_desktop::session::{self, Session};
use wugs_model::ulid;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let p = std::env::temp_dir().join(format!("wugs-drops-test-{}", ulid()));
        fs::create_dir_all(&p).unwrap();
        TempDir(p.canonicalize().unwrap())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn drop_dir(root: &Path) -> PathBuf {
    root.join("cache").join(drops::DROP_DIR)
}

fn leftovers(root: &Path) -> usize {
    fs::read_dir(drop_dir(root)).map(|d| d.count()).unwrap_or(0)
}

fn stage(drops: &Drops, root: &Path, name: &str, bytes: &[u8], piece: usize) -> drops::Staged {
    let id = drops.begin(root, name, bytes.len() as u64).unwrap();
    for chunk in bytes.chunks(piece) {
        drops.chunk(&id, chunk).unwrap();
    }
    drops.finish(&id).unwrap()
}

#[test]
fn a_file_arrives_whole_in_pieces_under_its_own_name() {
    let dir = TempDir::new();
    let drops = Drops::default();
    let bytes: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
    let staged = stage(&drops, &dir.0, "Frog story.eaf", &bytes, 4096);
    assert_eq!(staged.path.file_name().unwrap(), "Frog story.eaf");
    assert_eq!(fs::read(&staged.path).unwrap(), bytes);
    assert!(staged.path.starts_with(drop_dir(&dir.0)));
    assert_eq!(staged.root, dir.0);
}

#[test]
fn a_short_or_overlong_drop_is_refused_and_removed() {
    let dir = TempDir::new();
    let drops = Drops::default();

    let short = drops.begin(&dir.0, "a.wav", 10).unwrap();
    drops.chunk(&short, b"12345").unwrap();
    let err = drops.finish(&short).unwrap_err();
    assert!(err.contains("incomplete"), "{err}");

    let long = drops.begin(&dir.0, "b.wav", 4).unwrap();
    let err = drops.chunk(&long, b"12345").unwrap_err();
    assert!(err.contains("b.wav"), "{err}");
    assert!(drops.chunk(&long, b"1").is_err(), "the drop is over");

    let cancelled = drops.begin(&dir.0, "c.wav", 4).unwrap();
    drops.chunk(&cancelled, b"12").unwrap();
    drops.cancel(&cancelled);
    assert!(drops.finish(&cancelled).is_err());

    assert_eq!(leftovers(&dir.0), 0, "nothing is left in the cache");
}

#[test]
fn names_become_names_this_computer_can_create() {
    assert_eq!(drops::safe_name("Frog story.eaf"), "Frog story.eaf");
    assert_eq!(drops::safe_name("ʔisa dalawa.TextGrid"), "ʔisa dalawa.TextGrid");
    assert_eq!(drops::safe_name("a:b?.wav"), "a_b_.wav");
    assert_eq!(drops::safe_name("../../evil.wav"), "evil.wav");
    assert_eq!(drops::safe_name("..\\evil.wav"), "evil.wav");
    assert_eq!(drops::safe_name("con.wav"), "_con.wav");
    assert_eq!(drops::safe_name("COM1.eaf"), "_COM1.eaf");
    assert_eq!(drops::safe_name("console.wav"), "console.wav");
    assert_eq!(drops::safe_name("trailing. "), "trailing");
    assert_eq!(drops::safe_name(".."), "Dropped file");
    assert_eq!(drops::safe_name(""), "Dropped file");
    assert_eq!(drops::safe_name(&format!("{}.wav", "x".repeat(300))), format!("{}.wav", "x".repeat(120)));
}

#[test]
fn recordings_and_other_files_take_their_own_import() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio");
    assert_eq!(drops::route(&fixtures.join("tone.m4a")), Route::Recording);
    assert_eq!(drops::route(&fixtures.join("tone.flac")), Route::Recording);
    assert_eq!(drops::route(&fixtures.join("tone-mulaw.wav")), Route::Recording);
    assert_eq!(drops::route(Path::new("Frog.eaf")), Route::File);
    assert_eq!(drops::route(Path::new("lexicon.TXT")), Route::File);

    // A WAV is a recording by its first bytes, whatever its name.
    let dir = TempDir::new();
    let renamed = dir.0.join("take-1.dat");
    fs::copy(fixtures.join("tone-mulaw.wav"), &renamed).unwrap();
    assert_eq!(drops::route(&renamed), Route::Recording);
}

#[test]
fn a_dropped_textgrid_imports_like_a_picked_one() {
    let dir = TempDir::new();
    let mut a: Session = None;
    session::create_project(&mut a, dir.0.join("a").to_str().unwrap(), "prs_Test").unwrap();
    let text = session::create_text(&mut a, "Frog story").unwrap();
    session::add_segment(&mut a, &text, None, Some(0.0), Some(1200.0), "ʔisa dalawa").unwrap();
    let out = dir.0.join("out");
    fs::create_dir_all(&out).unwrap();
    let exported = interchange::export_text(&mut a, &text, Format::TextGrid, &out).unwrap();

    let mut b: Session = None;
    let b_root = dir.0.join("b");
    session::create_project(&mut b, b_root.to_str().unwrap(), "prs_Test").unwrap();
    let drops = Drops::default();
    let staged = stage(&drops, &session::project_root(&mut b).unwrap(), "Pear: story.TextGrid", &fs::read(&exported.path).unwrap(), 1000);
    assert_eq!(staged.path.file_name().unwrap(), "Pear_ story.TextGrid", "staged under a name Windows can create");
    let imported = drops::import_file(&mut b, &staged).unwrap();
    let shown = imported.text.expect("a text to show");
    let rows = session::list_texts(&mut b).unwrap();
    let row = rows.iter().find(|r| r.id == shown).unwrap();
    assert_eq!(row.title, "Pear: story", "titled from the name it was dropped with");
    assert_eq!(row.segments, 1);
}

#[test]
fn leftovers_a_crash_left_are_swept_after_an_hour() {
    let dir = TempDir::new();
    let base = drop_dir(&dir.0);
    let old = base.join("01OLD");
    let fresh = base.join("01FRESH");
    let live = base.join("01LIVE");
    for d in [&old, &fresh, &live] {
        fs::create_dir_all(d).unwrap();
        File::create(d.join("x.wav")).unwrap();
    }
    let two_hours_ago = SystemTime::now() - Duration::from_secs(7200);
    for d in [&old, &live] {
        File::options().write(true).open(d.join("x.wav")).unwrap().set_modified(two_hours_ago).unwrap();
    }
    drops::sweep(&base, |id| id == "01LIVE");
    assert!(!old.exists(), "an hour-old leftover goes");
    assert!(fresh.exists(), "a recent folder may still be written");
    assert!(live.exists(), "a drop being copied is never swept");
}

#[test]
fn a_drop_that_began_in_another_project_imports_nothing() {
    let dir = TempDir::new();
    let mut s: Session = None;
    let a = dir.0.join("a");
    session::create_project(&mut s, a.to_str().unwrap(), "prs_Test").unwrap();
    let text = session::create_text(&mut s, "Frog story").unwrap();
    session::add_segment(&mut s, &text, None, Some(0.0), Some(1200.0), "ʔisa").unwrap();
    let out = dir.0.join("out");
    fs::create_dir_all(&out).unwrap();
    let grid = interchange::export_text(&mut s, &text, Format::TextGrid, &out).unwrap();
    let wav = out.join("tone.wav");
    convert::to_wav(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio/tone.m4a"), &wav).unwrap();

    let drops = Drops::default();
    let a_root = session::project_root(&mut s).unwrap();
    let staged_grid = stage(&drops, &a_root, "Pear story.TextGrid", &fs::read(&grid.path).unwrap(), 1000);
    let staged_wav = stage(&drops, &a_root, "tone.wav", &fs::read(&wav).unwrap(), 1 << 16);

    // Project b opens while the files are still being copied or decoded.
    session::create_project(&mut s, dir.0.join("b").to_str().unwrap(), "prs_Test").unwrap();
    let err = drops::import_file(&mut s, &staged_grid).unwrap_err();
    assert!(err.contains("another project"), "{err}");
    let err = session::import_media_named(&mut s, &staged_wav.path, &staged_wav.name, Some(&staged_wav.root)).unwrap_err();
    assert!(err.contains("another project"), "{err}");
    assert!(session::list_texts(&mut s).unwrap().is_empty(), "no text landed in b");
    assert!(session::list_sources(&mut s).unwrap().is_empty(), "no recording landed in b");

    // Back in a, the same staged files import.
    session::open_project(&mut s, a.to_str().unwrap(), "prs_Test").unwrap();
    drops::import_file(&mut s, &staged_grid).unwrap();
    let row = session::import_media_named(&mut s, &staged_wav.path, &staged_wav.name, Some(&staged_wav.root)).unwrap();
    assert_eq!(row.label, "tone.wav");
}

#[test]
fn an_abandoned_drop_ends_at_the_next_one_and_huge_files_are_refused() {
    let dir = TempDir::new();
    let drops = Drops::default();
    let minutes = |m: u64| Instant::now() + Duration::from_secs(m * 60);
    let left = drops.begin(&dir.0, "half.wav", 10).unwrap();
    drops.chunk(&left, b"12345").unwrap();

    let next = drops.begin_at(&dir.0, "next.wav", 4, minutes(5)).unwrap();
    drops.chunk(&left, b"6").unwrap();
    assert_eq!(leftovers(&dir.0), 2, "five minutes of quiet is not abandoned");

    let later = drops.begin_at(&dir.0, "later.wav", 4, minutes(16)).unwrap();
    for id in [&left, &next] {
        assert!(drops.chunk(id, b"7").is_err(), "a drop quiet for ten minutes is over");
    }
    assert_eq!(leftovers(&dir.0), 1, "only the newest drop's folder stays");
    drops.cancel(&later);

    let err = drops.begin(&dir.0, "huge.wav", drops::MAX_BYTES + 1).unwrap_err();
    assert!(err.contains("huge.wav") && err.contains("8 GB"), "{err}");
    assert_eq!(leftovers(&dir.0), 0);
}

#[test]
fn a_link_dragged_onto_the_window_never_navigates_away() {
    let url = |s: &str| tauri::Url::parse(s).unwrap();
    for app in ["tauri://localhost/", "http://tauri.localhost/index.html", "https://tauri.localhost/"] {
        assert!(capture::is_app_origin(&url(app), None), "{app}");
    }
    for link in [
        "https://example.org/recording.wav",
        "http://localhost:1420/",
        "file:///C:/Users/x/Frog.eaf",
        "data:text/html,<h1>x</h1>",
        "about:blank",
        "https://tauri.localhost.example.org/",
    ] {
        assert!(!capture::is_app_origin(&url(link), None), "{link}");
    }
    let dev = url("http://localhost:1420");
    assert!(capture::is_app_origin(&url("http://localhost:1420/src/main.ts"), Some(&dev)), "the dev server in a debug build");
}
