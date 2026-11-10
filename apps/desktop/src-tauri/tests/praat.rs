use std::fs;
use std::path::{Path, PathBuf};

use wugs_desktop::praat;
use wugs_desktop::session::{self, Session};
use wugs_model::ulid;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let d = std::env::temp_dir().join(format!("wugs-praat-test-{}", ulid()));
        fs::create_dir_all(&d).unwrap();
        TempDir(d)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn wav(path: &Path, seconds: u32) {
    let rate = 16_000u32;
    let n = rate * seconds;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + n * 2).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(n * 2).to_le_bytes());
    b.resize(b.len() + (n * 2) as usize, 0);
    fs::write(path, b).unwrap();
}

/// A project with a 6 s recording and a text with "bir" and "iki".
fn project() -> (TempDir, Session, String, String) {
    let dir = TempDir::new();
    let mut s = Session::default();
    session::create_project(&mut s, dir.0.join("p").to_str().unwrap(), "prs_Ana").unwrap();
    let file = dir.0.join("rec.wav");
    wav(&file, 6);
    let source = session::import_media(&mut s, file.to_str().unwrap()).unwrap().id;
    let text = session::create_text(&mut s, "Story").unwrap();
    session::add_segment(&mut s, &text, None, Some(500.0), Some(1200.0), "bir").unwrap();
    session::add_segment(&mut s, &text, None, Some(2000.0), Some(2600.0), "iki").unwrap();
    (dir, s, text, source)
}

#[test]
fn versions_are_read_in_either_encoding_and_old_ones_refused() {
    assert_eq!(praat::parse_version(b"Praat 6.4.06 (February 25 2024)\n").unwrap(), "Praat 6.4.06 (February 25 2024)");
    let utf16: Vec<u8> = "Praat 6.1.38 (January 2 2021)\r\n".encode_utf16().flat_map(u16::to_le_bytes).collect();
    assert_eq!(praat::parse_version(&utf16).unwrap(), "Praat 6.1.38 (January 2 2021)", "Praat on Windows prints UTF-16");
    assert!(praat::parse_version(b"Praat 5.4.22 (2015)\n").unwrap_err().contains("too old"));
    assert!(praat::parse_version(b"bash: praat: command not found").is_err());
    assert!(praat::parse_version(b"").is_err());
}

#[test]
fn script_strings_are_quoted_the_praat_way() {
    assert_eq!(praat::praat_string(r#"C:\Users\A "B"\x.wav"#), r#""C:\Users\A ""B""\x.wav""#);
    let open = praat::open_script(Path::new("/p/a.wav"), Path::new("/p/grid.TextGrid"), "wugs_x", 1.25, 3.5);
    for line in ["Open long sound file: \"/p/a.wav\"", "Read from file: \"/p/grid.TextGrid\"", "View & Edit", "Zoom: 1.250, 3.500", "Extend time:"] {
        assert!(open.contains(line), "{line} in\n{open}");
    }
    let save = praat::save_script(Path::new("/p/grid.TextGrid"), "wugs_x");
    assert!(save.contains("selectObject: \"TextGrid wugs_x\"") && save.contains("Save as text file: \"/p/grid.TextGrid\""), "{save}");
}

#[test]
fn a_handoff_goes_out_comes_back_and_moves_its_baseline() {
    let (_dir, mut s, text, source) = project();
    assert!(!praat::pending(&mut s, &text).unwrap());
    let opened = praat::prepare(&mut s, &text, &source, 400.0, 2700.0).unwrap();
    assert_eq!(opened.not_shown, 0);
    let script = fs::read_to_string(&opened.script).unwrap();
    assert!(script.contains("Zoom: 0.400, 2.700"), "{script}");
    let folder = Path::new(&opened.script).parent().unwrap().to_path_buf();
    let audio = fs::read_dir(&folder).unwrap().map(|e| e.unwrap().path()).find(|p| p.extension().is_some_and(|x| x == "wav")).expect("the recording beside the grid");
    assert!(script.contains(&*audio.to_string_lossy()), "the script opens that file");
    assert!(praat::pending(&mut s, &text).unwrap());

    // Praat saves the grid with one label changed (as UTF-16, like Praat).
    let (grid, save) = praat::save_request(&mut s, &text).unwrap();
    assert!(fs::read_to_string(&save).unwrap().contains("Save as text file"));
    let edited = fs::read_to_string(&grid).unwrap().replace("\"iki\"", "\"ikí\"");
    let mut utf16 = vec![0xFE, 0xFF];
    utf16.extend(edited.encode_utf16().flat_map(u16::to_be_bytes));
    fs::write(&grid, utf16).unwrap();

    let view = praat::review(&mut s, &text).unwrap();
    assert_eq!((view.retimed, view.relabelled, view.added, view.removed), (0, 1, 0, 0));
    let applied = praat::apply(&mut s, &text).unwrap();
    assert_eq!(applied.changes, 1);
    let contents: Vec<String> = session::get_text(&mut s, &text).unwrap().segments.into_iter().map(|r| r.content).collect();
    assert!(contents.contains(&"ikí".to_string()), "{contents:?}");

    // Still open in Praat: the same save brings nothing back twice.
    let again = praat::review(&mut s, &text).unwrap();
    assert_eq!((again.retimed, again.relabelled, again.added, again.removed), (0, 0, 0, 0));

    praat::discard(&mut s, &text).unwrap();
    assert!(!praat::pending(&mut s, &text).unwrap());
    assert!(!folder.exists());
    praat::discard(&mut s, &text).unwrap();
    assert!(praat::review(&mut s, &text).is_err(), "nothing to bring back");
}

#[test]
fn texts_with_nothing_timed_and_bad_ids_are_refused() {
    let (_dir, mut s, _text, source) = project();
    let empty = session::create_text(&mut s, "Empty").unwrap();
    assert!(praat::prepare(&mut s, &empty, &source, 0.0, 1000.0).unwrap_err().contains("Add a segment"));
    assert!(praat::pending(&mut s, "../../etc").is_err(), "an id never names a folder outside the cache");
    assert!(praat::prepare(&mut s, "txt_x/..", &source, 0.0, 1.0).is_err());
}

#[test]
fn a_chosen_path_must_run_as_praat() {
    let dir = TempDir::new();
    assert!(praat::set_path(&dir.0, Some(dir.0.join("no-such-praat").to_str().unwrap())).is_err());
    assert!(!dir.0.join("praat.json").exists(), "a refused path is not saved");
    let back = praat::set_path(&dir.0, None).unwrap();
    assert!(!back.chosen);
}

/// Runs only where Praat is installed (not on CI).
#[test]
fn the_installed_praat_is_found_and_reads_what_we_write() {
    let st = praat::status(&TempDir::new().0);
    let Some(path) = st.path.filter(|_| st.problem.is_none()) else {
        eprintln!("no Praat here: {:?}", st.problem);
        return;
    };
    assert!(st.version.unwrap().starts_with("Praat "));

    // Everything the open script does except the window: load the linked
    // audio and our grid, extend it, then the save script's save.
    let (_dir, mut s, text, source) = project();
    let opened = praat::prepare(&mut s, &text, &source, 0.0, 1000.0).unwrap();
    let folder = Path::new(&opened.script).parent().unwrap().to_path_buf();
    let open = fs::read_to_string(&opened.script).unwrap();
    let headless: String = open.lines().take_while(|l| !l.starts_with("selectObject: sound, grid")).map(|l| format!("{l}\n")).collect();
    let (grid, save) = praat::save_request(&mut s, &text).unwrap();
    let both = format!("{headless}{}", fs::read_to_string(&save).unwrap());
    let run = folder.join("headless.praat");
    fs::write(&run, both).unwrap();
    let ok = std::process::Command::new(&path).arg("--run").arg(&run).status().unwrap();
    assert!(ok.success(), "Praat ran our scripts");
    let view = praat::review(&mut s, &text).unwrap();
    assert_eq!((view.retimed, view.relabelled, view.added, view.removed), (0, 0, 0, 0), "extending to the recording's end changes nothing: {view:?}");
    assert_ne!(fs::read(&grid).unwrap(), fs::read(folder.join("open.praat")).unwrap(), "Praat wrote the grid");
    assert!(fs::metadata(&grid).unwrap().len() > 0);
}

/// A stand-in for `praat --send`: a shell script that does what the save
/// script asks, or nothing.
#[cfg(unix)]
fn fake_praat(dir: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join("fake-praat");
    fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[cfg(unix)]
#[test]
fn a_save_is_seen_by_the_file_appearing_and_a_missing_one_puts_the_old_grid_back() {
    let dir = TempDir::new();
    let grid = dir.0.join("grid.TextGrid");
    let script = dir.0.join("save.praat");
    fs::write(&script, "").unwrap();

    // Praat saves the same bytes it was given (nothing edited): still seen.
    fs::write(&grid, "same").unwrap();
    let saves = fake_praat(&dir.0, &format!("printf same > '{}'", grid.display()));
    praat::ask_praat_to_save(&saves, &grid, &script).unwrap();
    assert_eq!(fs::read_to_string(&grid).unwrap(), "same");
    assert!(!grid.with_extension("previous").exists(), "the set-aside copy is cleaned up");

    let silent = fake_praat(&dir.0, "true");
    let started = std::time::Instant::now();
    assert!(praat::ask_praat_to_save(&silent, &grid, &script).unwrap_err().contains("did not save"));
    assert!(started.elapsed() >= praat::SAVE_WAIT);
    assert_eq!(fs::read_to_string(&grid).unwrap(), "same", "the old grid is back");
}

#[test]
fn unzipped_praat_folders_are_searched_newest_first() {
    let dir = TempDir::new();
    for d in ["praat6402_win-x64", "Praat7003_win-x64v3", "photos", "praat"] {
        fs::create_dir_all(dir.0.join(d)).unwrap();
    }
    fs::write(dir.0.join("praat-notes.txt"), "").unwrap();
    let names: Vec<String> = praat::praat_folders(&dir.0).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["Praat7003_win-x64v3", "praat6402_win-x64", "praat"]);
}
