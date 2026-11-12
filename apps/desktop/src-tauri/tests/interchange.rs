//! Import and export through the desktop session: what a person does with
//! the Texts panel's Import and Export buttons.

use std::fs;
use std::path::{Path, PathBuf};

use wugs_desktop::interchange::{self, Format};
use wugs_desktop::session::{self, Session};
use wugs_model::ulid;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let p = std::env::temp_dir().join(format!("wugs-interchange-test-{}", ulid()));
        fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn project(dir: &TempDir, name: &str) -> Session {
    let mut s: Session = None;
    session::create_project(&mut s, dir.0.join(name).to_str().unwrap(), "prs_Test").unwrap();
    s
}

fn texts(s: &mut Session) -> Vec<(String, String, usize)> {
    session::list_texts(s).unwrap().into_iter().map(|t| (t.id, t.title, t.segments)).collect()
}

#[test]
fn a_text_goes_out_and_comes_back_in_every_text_format() {
    let dir = TempDir::new();
    let mut a = project(&dir, "a");
    let text = session::create_text(&mut a, "Frog story / take 2").unwrap();
    session::add_segment(&mut a, &text, None, Some(0.0), Some(1200.0), "ʔisa dalawa").unwrap();
    session::add_segment(&mut a, &text, None, Some(1500.0), Some(2600.0), "tatlo").unwrap();
    let out = dir.0.join("out");
    fs::create_dir_all(&out).unwrap();

    for (format, ext) in [(Format::Elan, "eaf"), (Format::FlexText, "flextext"), (Format::TextGrid, "TextGrid")] {
        let first = interchange::export_text(&mut a, &text, format, &out).unwrap();
        assert_eq!(Path::new(&first.path), out.join(format!("Frog story _ take 2.{ext}")));
        let second = interchange::export_text(&mut a, &text, format, &out).unwrap();
        assert_eq!(Path::new(&second.path), out.join(format!("Frog story _ take 2 (2).{ext}")), "never replaces a file");

        let mut b = project(&dir, &format!("b-{ext}"));
        let imported = interchange::import_file(&mut b, &first.path).unwrap();
        let shown = imported.text.clone().expect("a text to show");
        let rows = texts(&mut b);
        let row = rows.iter().find(|r| r.0 == shown).unwrap_or_else(|| panic!("{ext}: imported text listed"));
        assert_eq!(row.2, 2, "{ext}: both segments arrived ({:?})", imported.lines);
        let detail = session::get_text(&mut b, &shown).unwrap();
        let contents: Vec<String> = detail.segments.iter().map(|s| s.content.clone()).collect();
        assert!(contents.contains(&"tatlo".to_string()), "{ext}: {contents:?}");

        let again = interchange::import_file(&mut b, &first.path).unwrap();
        assert_eq!(texts(&mut b).iter().map(|r| r.2).sum::<usize>(), 2, "{ext}: re-import adds nothing ({:?})", again.lines);
    }
}

#[test]
fn the_lexicon_goes_out_and_comes_back_as_lift_and_toolbox() {
    let dir = TempDir::new();
    let mut a = project(&dir, "a");
    a.as_mut().unwrap().add_lexeme("aso", "dog").unwrap();
    a.as_mut().unwrap().add_lexeme("pusa", "cat").unwrap();
    let out = dir.0.join("out");
    fs::create_dir_all(&out).unwrap();
    for format in [Format::Lift, Format::Toolbox] {
        let file = interchange::export_lexicon(&mut a, format, &out).unwrap();
        let mut b = project(&dir, &format!("b-{format:?}"));
        let imported = interchange::import_file(&mut b, &file.path).unwrap();
        assert!(imported.lexicon && imported.text.is_none());
        let lexemes = b.as_ref().unwrap().proj.entities_of_kind("lexeme").unwrap();
        assert_eq!(lexemes.len(), 2, "{format:?}: {:?}", imported.lines);
    }
    assert!(interchange::export_lexicon(&mut a, Format::Elan, &out).is_err());
}

#[test]
fn sealing_makes_a_new_package_each_time() {
    let dir = TempDir::new();
    let mut a = project(&dir, "a");
    let text = session::create_text(&mut a, "Elicitation").unwrap();
    session::add_segment(&mut a, &text, None, Some(0.0), Some(900.0), "isa").unwrap();
    let out = dir.0.join("out");
    fs::create_dir_all(&out).unwrap();
    let first = interchange::seal_project(&mut a, &out, "2026-11-11").unwrap();
    assert_eq!(Path::new(&first.path), out.join("wugs-sealed-2026-11-11"));
    assert_eq!(first.texts, 1);
    assert!(out.join("wugs-sealed-2026-11-11/bagit.txt").is_file());
    let second = interchange::seal_project(&mut a, &out, "2026-11-11").unwrap();
    assert_eq!(Path::new(&second.path), out.join("wugs-sealed-2026-11-11-2"));
}

#[test]
fn a_plain_text_comes_in_a_sentence_per_segment_and_goes_out_a_line_per_segment() {
    let dir = TempDir::new();
    let mut a = project(&dir, "a");
    let f = dir.0.join("Fox essay.txt");
    fs::write(&f, "The fox came. It drank from the well!\n\nThen it left.\n").unwrap();
    let imported = interchange::import_file(&mut a, f.to_str().unwrap()).unwrap();
    let text = imported.text.clone().unwrap();
    let detail = session::get_text(&mut a, &text).unwrap();
    assert_eq!(detail.title, "Fox essay");
    let contents: Vec<String> = detail.segments.iter().map(|s| s.content.clone()).collect();
    assert_eq!(contents, ["The fox came.", "It drank from the well!", "Then it left."], "{:?}", imported.lines);
    // The same file again adds nothing.
    let again = interchange::import_file(&mut a, f.to_str().unwrap()).unwrap();
    assert_eq!(again.text.as_deref(), Some(text.as_str()));
    assert_eq!(texts(&mut a).len(), 1);
    // Out as a line per segment, which reads back as the same text.
    let out = dir.0.join("out");
    fs::create_dir_all(&out).unwrap();
    let file = interchange::export_text(&mut a, &text, Format::PlainText, &out).unwrap();
    assert_eq!(fs::read_to_string(&file.path).unwrap(), "The fox came.\nIt drank from the well!\nThen it left.\n");
    let mut b = project(&dir, "b");
    let back = interchange::import_file(&mut b, &file.path).unwrap();
    assert_eq!(session::get_text(&mut b, &back.text.unwrap()).unwrap().segments.len(), 3);
    // Export goes in time order, whatever order the segments were drawn in.
    let drawn = session::create_text(&mut a, "Drawn").unwrap();
    session::add_segment(&mut a, &drawn, None, Some(5000.0), Some(6000.0), "second in time").unwrap();
    session::add_segment(&mut a, &drawn, None, Some(0.0), Some(1000.0), "first in time").unwrap();
    let file = interchange::export_text(&mut a, &drawn, Format::PlainText, &out).unwrap();
    assert_eq!(fs::read_to_string(&file.path).unwrap(), "first in time\nsecond in time\n");
    // An NFD file name and text match their NFC import.
    let nfd = dir.0.join("Cafe\u{301}.txt");
    fs::write(&nfd, "Cafe\u{301} au lait.\n").unwrap();
    let one = interchange::import_file(&mut a, nfd.to_str().unwrap()).unwrap().text;
    let two = interchange::import_file(&mut a, nfd.to_str().unwrap()).unwrap().text;
    assert_eq!(one, two, "re-import matched in NFC");
    // UTF-16 with no byte-order mark is refused, not stored as NULs.
    let utf16 = dir.0.join("u16.txt");
    fs::write(&utf16, "Hello there.".encode_utf16().flat_map(|u| u.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
    assert!(interchange::import_file(&mut a, utf16.to_str().unwrap()).is_err());
    // A damaged file is refused, not garbled.
    let bad = dir.0.join("bad.txt");
    fs::write(&bad, b"caf\xE9").unwrap();
    assert!(interchange::import_file(&mut a, bad.to_str().unwrap()).is_err());
}

#[test]
fn an_unknown_file_is_refused_in_words() {
    let dir = TempDir::new();
    let mut a = project(&dir, "a");
    let f = dir.0.join("notes.md");
    fs::write(&f, "just some notes").unwrap();
    let err = interchange::import_file(&mut a, f.to_str().unwrap()).unwrap_err();
    assert!(err.contains("not a file wugs can import"), "{err}");
}

#[test]
fn nothing_is_written_inside_the_project_folder() {
    let dir = TempDir::new();
    let mut a = project(&dir, "a");
    let text = session::create_text(&mut a, "Elicitation").unwrap();
    let blobs = dir.0.join("a").join("blobs");
    fs::create_dir_all(&blobs).unwrap();
    for folder in [dir.0.join("a"), blobs] {
        let err = interchange::seal_project(&mut a, &folder, "2026-11-11").unwrap_err();
        assert!(err.contains("inside the project folder"), "{err}");
        assert!(interchange::export_text(&mut a, &text, Format::Elan, &folder).is_err());
        assert!(interchange::export_lexicon(&mut a, Format::Lift, &folder).is_err());
    }
}

#[test]
fn a_table_of_sentences_comes_in_a_text_per_document() {
    let dir = TempDir::new();
    let mut a = project(&dir, "a");
    let f = dir.0.join("corpus.csv");
    fs::write(&f, "doc,speaker,sentence\nFox,Maria,\"The fox came, slowly.\"\nFox,Jo,It drank.\nWell,Maria,The well was dry.\n").unwrap();
    let imported = interchange::import_file(&mut a, f.to_str().unwrap()).unwrap();
    assert!(imported.lines.iter().any(|l| l.contains("2 new texts") && l.contains("«sentence»") && l.contains("«doc»")), "{:?}", imported.lines);
    assert!(imported.lines.iter().any(|l| l.contains("«speaker»")), "{:?}", imported.lines);
    let all = texts(&mut a);
    assert_eq!(all.len(), 2);
    let fox = all.iter().find(|t| t.1 == "Fox").expect("a text per document").0.clone();
    let detail = session::get_text(&mut a, &fox).unwrap();
    let contents: Vec<String> = detail.segments.iter().map(|s| s.content.clone()).collect();
    assert_eq!(contents, ["The fox came, slowly.", "It drank."]);
    let p = a.as_mut().unwrap();
    assert_eq!(p.proj.field_str(&detail.segments[1].id, "x.csv.speaker").unwrap().as_deref(), Some("Jo"), "other columns are kept");
    // The same file again adds nothing.
    let again = interchange::import_file(&mut a, f.to_str().unwrap()).unwrap();
    assert!(again.lines.iter().any(|l| l.contains("2 documents matched")), "{:?}", again.lines);
    assert_eq!(texts(&mut a).len(), 2);
}
