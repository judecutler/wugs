use std::fs;
use std::path::PathBuf;

use wugs_desktop::capture::{self, Captures};
use wugs_desktop::session::{self, SearchScope, Session};
use wugs_model::{ulid, Op};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        TempDir(std::env::temp_dir().join(format!("wugs-desktop-test-{}", ulid())))
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn created(actor: &str) -> (TempDir, Session) {
    let dir = TempDir::new();
    let mut s: Session = None;
    session::create_project(&mut s, dir.path(), actor).unwrap();
    (dir, s)
}

fn own_ops(s: &Session) -> Vec<Op> {
    let p = s.as_ref().unwrap();
    let me = p.device().to_string();
    p.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == me).collect()
}

fn pull(into: &mut Session, from: &Session) {
    let r = into.as_mut().unwrap().ingest(own_ops(from)).unwrap();
    assert_eq!(r.txns_held, 0, "nothing held: {:?}", r.held);
}

fn content(s: &mut Session, text: &str, seg: &str) -> String {
    let detail = session::get_text(s, text).unwrap();
    detail.segments.into_iter().find(|r| r.id == seg).unwrap().content
}

#[test]
fn create_then_reopen_keeps_the_project() {
    let (dir, mut s) = created("prs_Alice");
    let id = s.as_ref().unwrap().project_id.clone();
    session::create_text(&mut s, "Elicitation 1").unwrap();

    let mut reopened: Session = None;
    drop(s);
    let info = session::open_project(&mut reopened, dir.path(), "prs_Alice").unwrap();
    assert_eq!(info.project_id, id, "same project after reopen");
    assert_eq!(info.texts, 1, "the text survived the reopen");
    assert_eq!(info.conflicts, 0);
}

#[test]
fn create_refuses_an_existing_project() {
    let (dir, mut s) = created("prs_Alice");
    let err = session::create_project(&mut s, dir.path(), "prs_Alice").unwrap_err();
    assert!(err.contains("already holds"), "{err}");
}

#[test]
fn open_refuses_a_directory_without_a_project_and_creates_nothing() {
    let dir = TempDir::new();
    fs::create_dir_all(&dir.0).unwrap();
    let mut s: Session = None;
    let err = session::open_project(&mut s, dir.path(), "prs_Alice").unwrap_err();
    assert!(err.contains("project.json"), "{err}");
    assert!(s.is_none(), "no session after a failed open");
    assert!(!dir.0.join("project.json").exists(), "open must not initialize a project");
}

#[test]
fn open_requires_a_researcher() {
    let dir = TempDir::new();
    let mut s: Session = None;
    assert!(session::create_project(&mut s, dir.path(), "  ").is_err());
    assert!(!dir.0.join("project.json").exists());
}

#[test]
fn commands_without_a_project_are_errors() {
    let mut s: Session = None;
    assert!(session::list_texts(&mut s).is_err());
    assert!(session::create_text(&mut s, "T").is_err());
    assert!(session::conflict_count(&mut s).is_err());
}

#[test]
fn texts_and_segments_round_trip() {
    let (_dir, mut s) = created("prs_Alice");
    let txt = session::create_text(&mut s, "Frog story").unwrap();
    let a = session::add_segment(&mut s, &txt, None, Some(0.0), Some(1200.0), "köpek geldi").unwrap().id;
    let b = session::add_segment(&mut s, &txt, None, Some(1200.0), Some(2500.0), "kedi gitti").unwrap().id;

    let texts = session::list_texts(&mut s).unwrap();
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0].title, "Frog story");
    assert_eq!(texts[0].segments, 2);

    let detail = session::get_text(&mut s, &txt).unwrap();
    let ids: Vec<&str> = detail.segments.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, [a.as_str(), b.as_str()], "segments come back in pos order");
    assert_eq!(detail.segments[1].start_ms, Some(1200.0));
    assert_eq!(detail.segments[1].end_ms, Some(2500.0));
    assert_eq!(detail.segments[0].content, "köpek geldi");

    assert_eq!(detail.tiers.len(), 1, "both segments share one default tier");
    assert_eq!(detail.tiers[0].name, "utterance");
    assert_eq!(detail.tiers[0].kind.as_deref(), Some("time-aligned"));
    assert_eq!(detail.segments[0].tier, detail.tiers[0].id);
}

#[test]
fn default_tier_is_reused_across_texts_and_reopens() {
    let (dir, mut s) = created("prs_Alice");
    let t1 = session::create_text(&mut s, "One").unwrap();
    session::add_segment(&mut s, &t1, None, None, None, "a").unwrap();
    drop(s);

    let mut s: Session = None;
    session::open_project(&mut s, dir.path(), "prs_Alice").unwrap();
    let t2 = session::create_text(&mut s, "Two").unwrap();
    session::add_segment(&mut s, &t2, None, None, None, "b").unwrap();

    let tier1 = session::get_text(&mut s, &t1).unwrap().tiers[0].id.clone();
    let tier2 = session::get_text(&mut s, &t2).unwrap().tiers[0].id.clone();
    assert_eq!(tier1, tier2, "one project-wide utterance tier");
    assert_eq!(s.as_ref().unwrap().proj.entities_of_kind("tier").unwrap().len(), 1);
}

#[test]
fn get_text_rejects_an_id_that_is_not_a_text() {
    let (_dir, mut s) = created("prs_Alice");
    let txt = session::create_text(&mut s, "T").unwrap();
    let seg = session::add_segment(&mut s, &txt, None, None, None, "x").unwrap().id;
    assert!(session::get_text(&mut s, &seg).is_err());
    assert!(session::get_text(&mut s, "txt_missing").is_err());
}

#[test]
fn editing_plain_created_content_returns_the_merged_text() {
    let (dir, mut s) = created("prs_Alice");
    let txt = session::create_text(&mut s, "T").unwrap();
    // A plain `create` with content, the shape every importer writes: the
    // first text-edit has to seed a CRDT genesis from it.
    let seg = session::add_segment(&mut s, &txt, None, None, None, "köpek geldi").unwrap().id;

    let first = session::edit_segment_content(&mut s, &txt, &seg, "köpekler geldi").unwrap();
    assert_eq!(first.content, "köpekler geldi");
    assert!(first.findings.is_empty(), "{:?}", first.findings);
    let second = session::edit_segment_content(&mut s, &txt, &seg, "köpekler geldi dün").unwrap();
    assert_eq!(second.content, "köpekler geldi dün");
    assert_eq!(content(&mut s, &txt, &seg), second.content, "the returned value is what get_text reads");

    drop(s);
    fs::remove_dir_all(dir.0.join("cache")).unwrap();
    let mut s: Session = None;
    session::open_project(&mut s, dir.path(), "prs_Alice").unwrap();
    assert_eq!(content(&mut s, &txt, &seg), "köpekler geldi dün", "a cache rebuild agrees");
}

#[test]
fn two_sessions_converge_after_concurrent_edits() {
    let (_da, mut alice) = created("prs_Alice");
    let (_db, mut bob) = created("prs_Bob");
    let txt = session::create_text(&mut alice, "Shared").unwrap();
    let seg = session::add_segment(&mut alice, &txt, None, None, None, "köpek geldi").unwrap().id;
    pull(&mut bob, &alice);
    assert_eq!(content(&mut bob, &txt, &seg), "köpek geldi");

    session::edit_segment_content(&mut alice, &txt, &seg, "büyük köpek geldi").unwrap();
    session::edit_segment_content(&mut bob, &txt, &seg, "köpek geldi dün").unwrap();
    pull(&mut alice, &bob);
    pull(&mut bob, &alice);

    let merged = content(&mut alice, &txt, &seg);
    assert_eq!(merged, "büyük köpek geldi dün", "both edits survive");
    assert_eq!(content(&mut bob, &txt, &seg), merged, "bob converges");
    assert_eq!(session::conflict_count(&mut alice).unwrap(), 0);
    assert_eq!(session::conflict_count(&mut bob).unwrap(), 0);

    let after = session::edit_segment_content(&mut bob, &txt, &seg, "büyük köpek geldi dün akşam").unwrap();
    assert_eq!(after.content, "büyük köpek geldi dün akşam", "editing on top of a merge applies verbatim");
}

#[test]
fn session_thread_runs_commands_and_closes_the_project_on_a_panic() {
    let dir = TempDir::new();
    let path = dir.path().to_string();
    let thread = wugs_desktop::SessionThread::spawn();
    thread.run(move |s| session::create_project(s, &path, "prs_Alice")).unwrap();
    thread.run(|s| session::create_text(s, "T")).unwrap();
    assert_eq!(thread.run(session::list_texts).unwrap().len(), 1);

    let err = thread.run(|_| -> Result<(), String> { panic!("injected") }).unwrap_err();
    assert!(err.contains("closed"), "{err}");
    let err = thread.run(session::list_texts).unwrap_err();
    assert_eq!(err, "no project is open", "the thread survives and the project is closed");
}

#[test]
fn a_failed_open_keeps_the_current_project() {
    let (_dir, mut s) = created("prs_Alice");
    let id = s.as_ref().unwrap().project_id.clone();
    let missing = TempDir::new();
    assert!(session::open_project(&mut s, missing.path(), "prs_Alice").is_err());
    assert_eq!(s.as_ref().unwrap().project_id, id, "the open project is untouched");
    assert!(session::list_texts(&mut s).is_ok());
}

#[test]
fn bad_ids_write_nothing_to_the_log() {
    let (dir, mut s) = created("prs_Alice");
    let txt = session::create_text(&mut s, "T").unwrap();
    let seg = session::add_segment(&mut s, &txt, None, None, None, "köpek geldi").unwrap().id;
    let other = session::create_text(&mut s, "Other").unwrap();
    let scopes_before = std::fs::read_dir(format!("{}/log", dir.path())).unwrap().count();

    let err = session::add_segment(&mut s, "txt_01JUNKJUNKJUNKJUNKJUNK00", None, None, None, "x");
    assert!(err.is_err(), "segment into a missing text must fail");

    let err = session::edit_segment_content(&mut s, &txt, "seg_01JUNKJUNKJUNKJUNKJUNK00", "x");
    assert!(err.is_err(), "editing a missing segment must fail");

    let err = session::edit_segment_content(&mut s, &other, &seg, "x");
    assert!(err.is_err(), "a segment edited under the wrong text would land in the wrong scope log");

    let scopes_after = std::fs::read_dir(format!("{}/log", dir.path())).unwrap().count();
    assert_eq!(scopes_after, scopes_before, "no scope directory appeared for a bogus id");
    assert_eq!(content(&mut s, &txt, &seg), "köpek geldi", "the real segment is untouched");
}

#[test]
fn new_segments_sort_after_imported_or_deleted_ones() {
    let (_dir, mut s) = created("prs_Alice");
    let txt = session::create_text(&mut s, "T").unwrap();
    session::add_segment(&mut s, &txt, None, None, None, "one").unwrap();
    session::add_segment(&mut s, &txt, None, None, None, "two").unwrap();
    let detail = session::get_text(&mut s, &txt).unwrap();
    assert_eq!(detail.segments[0].content, "one");
    assert_eq!(detail.segments[1].content, "two");
    assert!(detail.segments[0].pos < detail.segments[1].pos, "positions strictly increase");

    // simulate an import that used a high pos, then keep appending
    {
        let p = s.as_mut().unwrap();
        let mut t = p.intent("import-sim");
        t.create(
            "segment",
            "seg",
            &txt.clone(),
            vec![
                ("text", wugs_model::Value::str(&txt)),
                ("tier", wugs_model::Value::str(&detail.segments[0].tier)),
                ("pos", wugs_model::Value::str("000000ff")),
                ("content", wugs_model::Value::str("imported")),
            ],
        );
        t.commit().unwrap();
    }
    session::add_segment(&mut s, &txt, None, None, None, "after import").unwrap();
    let detail = session::get_text(&mut s, &txt).unwrap();
    let contents: Vec<&str> = detail.segments.iter().map(|r| r.content.as_str()).collect();
    assert_eq!(contents, ["one", "two", "imported", "after import"], "a new segment never collides with a taken pos");
}

#[test]
fn sources_and_search_reach_the_shell() {
    let (_dir, mut s) = created("prs_Alice");
    s.as_mut().unwrap().add_source("Elicitation tape 3", None, "audio", Some("restricted")).unwrap();
    let text = session::create_text(&mut s, "Frog story").unwrap();
    session::add_segment(&mut s, &text, None, None, None, "köpek geldi").unwrap();

    let sources = session::list_sources(&mut s).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].label, "Elicitation tape 3");
    assert_eq!(sources[0].access.as_deref(), Some("restricted"));
    assert!(!sources[0].has_media);

    let hits = session::search(&mut s, "kopek", SearchScope::All).unwrap();
    let seg = hits.iter().find(|h| h.kind == "segment").expect("the segment matches with diacritics folded");
    assert_eq!(seg.text_id.as_deref(), Some(text.as_str()), "a segment hit names its text");
    assert!(session::search(&mut s, "   ", SearchScope::All).unwrap().is_empty());
}

#[test]
fn search_rows_locate_segments_and_filter_by_kind() {
    let (_dir, mut s) = created("prs_Alice");
    s.as_mut().unwrap().create_lexeme("köpek", "dog").unwrap();
    let text = session::create_text(&mut s, "Frog story").unwrap();
    session::add_segment(&mut s, &text, None, Some(65_000.0), Some(66_000.0), "köpek geldi").unwrap();
    let detail = session::get_text(&mut s, &text).unwrap();

    let all = session::search(&mut s, "kopek", SearchScope::All).unwrap();
    let seg = all.iter().find(|h| h.kind == "segment").expect("segment hit");
    assert_eq!(seg.tier_id.as_deref(), Some(detail.segments[0].tier.as_str()), "a segment hit names its tier");
    assert_eq!(seg.tier_name.as_deref(), Some(detail.tiers[0].name.as_str()));
    assert_eq!(seg.text_title.as_deref(), Some("Frog story"));
    assert_eq!(seg.start_ms, Some(65_000.0));
    assert_eq!(seg.end_ms, Some(66_000.0), "the end lets Search play the hit");
    assert!(all.iter().any(|h| h.kind == "lexeme"), "All includes the lexicon");

    let texts = session::search(&mut s, "kopek", SearchScope::Texts).unwrap();
    assert!(!texts.is_empty() && texts.iter().all(|h| h.kind == "segment" || h.kind == "text"), "{texts:?}");
    let lex = session::search(&mut s, "kopek", SearchScope::Lexicon).unwrap();
    assert!(!lex.is_empty() && lex.iter().all(|h| h.kind == "lexeme" || h.kind == "sense"), "{lex:?}");
    assert!(lex.iter().all(|h| h.text_id.is_none() && h.tier_id.is_none()), "lexicon hits are not navigable");
}

#[test]
fn search_treats_fts_operators_as_literal_text() {
    let (_dir, mut s) = created("prs_Alice");
    let text = session::create_text(&mut s, "Operators").unwrap();
    session::add_segment(&mut s, &text, None, None, None, "alpha beta").unwrap();
    session::add_segment(&mut s, &text, None, None, None, "say \"hi\" now").unwrap();
    session::add_segment(&mut s, &text, None, None, None, "NEAR gamma").unwrap();
    session::add_segment(&mut s, &text, None, None, None, "non-stop").unwrap();
    let mut count = |q: &str| session::search(&mut s, q, SearchScope::All).unwrap().iter().filter(|h| h.kind == "segment").count();

    assert_eq!(count("alpha OR beta"), 0, "OR is a word, not a union");
    assert_eq!(count("NEAR"), 1);
    assert_eq!(count("NEAR(alpha beta)"), 0, "NEAR( is not a proximity query");
    assert_eq!(count("\"hi\""), 1, "quotes are data");
    assert_eq!(count("\"hi"), 1, "an unbalanced quote does not error");
    assert_eq!(count("-stop"), 1, "a leading minus is not NOT");
    assert_eq!(count("alp*"), 1);
    assert_eq!(count("alpha -beta"), 1, "minus does not exclude: the segment containing beta still matches");
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn copied_project() -> (TempDir, TempDir) {
    let (dir, s) = created("prs_Alice");
    drop(s);
    let copy = TempDir::new();
    copy_dir(&dir.0, &copy.0);
    (dir, copy)
}

#[test]
fn opening_a_copied_folder_reports_where_it_came_from_and_blocks_nothing() {
    let (dir, copy) = copied_project();
    let mut s: Session = None;
    let info = session::open_project(&mut s, copy.path(), "prs_Alice").unwrap();
    let reloc = info.relocation.expect("the copy is reported at open");
    // The banner path is display-stripped (no Windows \\?\ prefix), so the
    // expectation goes through the same function.
    let expected = session::display_path(&fs::canonicalize(&dir.0).unwrap().to_string_lossy());
    assert_eq!(PathBuf::from(&reloc.from), PathBuf::from(expected));
    assert!(reloc.original_still_present);

    let text = session::create_text(&mut s, "Works while the banner is up").unwrap();
    session::add_segment(&mut s, &text, None, Some(0.0), Some(500.0), "köpek").unwrap();
    drop(s);

    let mut original: Session = None;
    assert!(session::open_project(&mut original, dir.path(), "prs_Alice").unwrap().relocation.is_none());
}

#[test]
fn accepting_a_move_clears_it_on_the_next_open_and_keeps_the_device_id() {
    let (_dir, copy) = copied_project();
    let mut s: Session = None;
    session::open_project(&mut s, copy.path(), "prs_Alice").unwrap();
    let device = s.as_ref().unwrap().device().to_string();
    session::relocation_accept_move(&mut s).unwrap();
    drop(s);

    let mut again: Session = None;
    assert!(session::open_project(&mut again, copy.path(), "prs_Alice").unwrap().relocation.is_none());
    assert_eq!(again.as_ref().unwrap().device(), device);
}

#[test]
fn a_fresh_device_identity_clears_the_relocation_on_the_next_open() {
    let (_dir, copy) = copied_project();
    let mut s: Session = None;
    session::open_project(&mut s, copy.path(), "prs_Alice").unwrap();
    let device = s.as_ref().unwrap().device().to_string();
    session::mint_device_identity(&mut s, &wugs_auth::DeviceKey::generate()).unwrap();
    drop(s);

    let mut again: Session = None;
    assert!(session::open_project(&mut again, copy.path(), "prs_Alice").unwrap().relocation.is_none());
    assert_ne!(again.as_ref().unwrap().device(), device);
}

#[test]
fn display_path_drops_the_verbatim_prefix_and_keeps_unc_shares() {
    assert_eq!(session::display_path(r"\\?\C:\field\lezgi"), r"C:\field\lezgi");
    assert_eq!(session::display_path(r"\\?\UNC\server\share\lezgi"), r"\\server\share\lezgi");
    assert_eq!(session::display_path("/home/a/lezgi"), "/home/a/lezgi");
}

// ------------------------------------------------------------ media, peaks

/// A small PCM16 WAV: `frames` frames of `channels` channels at `rate` Hz,
/// each sample a ramp so peaks are nonzero and channels differ.
fn write_wav(path: &std::path::Path, channels: u16, rate: u32, frames: u32) {
    let block = channels * 2;
    let data_len = frames * block as u32;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * block as u32).to_le_bytes());
    bytes.extend_from_slice(&block.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for f in 0..frames {
        for c in 0..channels {
            let s = ((f as i32 % 2000) - 1000 + c as i32 * 100) as i16;
            bytes.extend_from_slice(&s.to_le_bytes());
        }
    }
    fs::write(path, bytes).unwrap();
}

fn only_file(dir: &std::path::Path) -> PathBuf {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    assert_eq!(entries.len(), 1, "exactly one file under {}: {entries:?}", dir.display());
    entries.pop().unwrap()
}

#[test]
fn import_media_rejects_anything_but_wav() {
    let (dir, mut s) = created("prs_Alice");
    let bogus = dir.0.join("fieldnotes.txt");
    fs::write(&bogus, "not audio").unwrap();
    let err = session::import_media(&mut s, bogus.to_str().unwrap()).unwrap_err();
    assert!(err.contains("WAV"), "the error names WAV: {err}");
    assert!(session::list_sources(&mut s).unwrap().is_empty(), "nothing was created");
}

#[test]
fn import_media_roundtrips_through_peaks_queries() {
    let (dir, mut s) = created("prs_Alice");
    let wav = dir.0.join("Elicitation 03.wav");
    write_wav(&wav, 2, 8000, 16_000); // 2 s stereo

    let row = session::import_media(&mut s, wav.to_str().unwrap()).unwrap();
    assert_eq!(row.label, "Elicitation 03.wav");
    assert_eq!(row.kind.as_deref(), Some("audio"));
    assert!(row.has_media);
    let listed = session::list_sources(&mut s).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].has_media, "track-attached media counts as media");

    let info = session::peaks_info(&mut s, &row.id).unwrap();
    assert_eq!((info.frames, info.sample_rate, info.channels), (16_000, 8000, 2));
    assert_eq!(info.duration_ms, 2000.0);
    assert!(std::path::Path::new(&info.media_path).is_file(), "media path resolves: {}", info.media_path);

    // The source carries its duration, so validator #7 has something to check.
    let p = s.as_ref().unwrap();
    assert_eq!(p.proj.field_num(&row.id, "duration").unwrap(), Some(2000.0));

    let slice = session::peaks_query(&mut s, &row.id, 0, 0, 16_000, 400).unwrap();
    assert_eq!(slice.mins.len(), slice.maxs.len());
    assert!(!slice.mins.is_empty());
    assert_eq!(slice.first_frame, 0);
    assert!(slice.mins.iter().zip(&slice.maxs).all(|(lo, hi)| lo <= hi));
    let other = session::peaks_query(&mut s, &row.id, 1, 0, 16_000, 400).unwrap();
    assert_ne!(slice.maxs, other.maxs, "channels differ in the test signal");

    assert!(session::peaks_query(&mut s, &row.id, 2, 0, 100, 10).is_err(), "no third channel");
    assert!(session::peaks_query(&mut s, &row.id, 0, 0, 100, 0).is_err(), "zero pixels refused");
}

#[test]
fn peaks_come_from_the_wpk_cache_once_it_exists() {
    let (dir, mut s) = created("prs_Alice");
    let wav = dir.0.join("one.wav");
    write_wav(&wav, 1, 8000, 8000);
    let row = session::import_media(&mut s, wav.to_str().unwrap()).unwrap();

    let wpk = only_file(&dir.0.join("cache").join("peaks"));
    assert_eq!(wpk.extension().and_then(|e| e.to_str()), Some("wpk"));
    let blob = only_file(&dir.0.join("blobs"));
    assert_eq!(wpk.file_stem().unwrap(), blob.file_name().unwrap(), "cache keyed by the blob hash");

    // A reopened session has an empty in-memory cache; with the blob file
    // gone, only the .wpk can answer.
    drop(s);
    fs::remove_file(&blob).unwrap();
    let mut s: Session = None;
    session::open_project(&mut s, dir.path(), "prs_Alice").unwrap();
    let info = session::peaks_info(&mut s, &row.id).unwrap();
    assert_eq!(info.frames, 8000, "answered from cache/peaks, not the audio file");
    session::peaks_query(&mut s, &row.id, 0, 0, 8000, 100).unwrap();
}

#[test]
fn timeline_edits_retiming_deleting_and_new_tiers() {
    let (_dir, mut s) = created("prs_Alice");
    let txt = session::create_text(&mut s, "Frog story").unwrap();
    let a = session::add_segment(&mut s, &txt, None, Some(0.0), Some(1000.0), "bir").unwrap();
    assert!(a.findings.is_empty(), "{:?}", a.findings);
    let b = session::add_segment(&mut s, &txt, None, Some(500.0), Some(1500.0), "iki").unwrap();
    assert!(b.findings.iter().any(|f| f.contains("tier-overlap")), "adds report findings: {:?}", b.findings);

    let findings = session::set_segment_times(&mut s, &txt, &b.id, Some(1000.0), Some(1500.0)).unwrap();
    assert!(findings.is_empty(), "{findings:?}");
    assert!(session::set_segment_times(&mut s, &txt, &b.id, Some(-1.0), Some(5.0)).is_err());
    assert!(session::set_segment_times(&mut s, &txt, &b.id, Some(7.0), Some(5.0)).is_err());
    assert!(session::set_segment_times(&mut s, &txt, &b.id, Some(1600.0), None).is_err(), "start past the stored end");
    assert!(session::set_segment_times(&mut s, &txt, &b.id, None, Some(900.0)).is_err(), "end before the stored start");

    let detail = session::get_text(&mut s, &txt).unwrap();
    let row = detail.segments.iter().find(|r| r.id == b.id).unwrap();
    assert_eq!((row.start_ms, row.end_ms), (Some(1000.0), Some(1500.0)));

    session::delete_segment(&mut s, &txt, &a.id).unwrap();
    let detail = session::get_text(&mut s, &txt).unwrap();
    assert!(detail.segments.iter().all(|r| r.id != a.id), "deleted segment left the timeline");

    let tier = session::add_tier(&mut s, "translation", None).unwrap();
    assert_eq!((tier.name.as_str(), tier.role.as_deref()), ("translation", Some("translation")));
    let note = session::add_tier(&mut s, "notes", None).unwrap();
    assert_eq!(note.role.as_deref(), Some("note"), "the notes template carries the note role");
    assert!(session::add_tier(&mut s, "syntax-tree", None).is_err(), "not a template");
    assert!(session::add_tier(&mut s, "gloss", None).is_err(), "glosses live on morphs, not on a track");
    let named = session::add_tier(&mut s, "custom", Some("Kera dialect")).unwrap();
    assert_eq!((named.name.as_str(), named.role.as_deref()), ("Kera dialect", None));

    let detail = session::get_text(&mut s, &txt).unwrap();
    let mut names: Vec<&str> = detail.tiers.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names[0], "utterance", "the first lane is the one the segments use");
    names.sort();
    assert_eq!(names, ["Kera dialect", "notes", "translation", "utterance"], "lanes include empty tiers");
    let lane = |n: &str| detail.tiers.iter().find(|t| t.name == n).unwrap();
    assert_eq!(lane("translation").role.as_deref(), Some("translation"), "get_text exposes the role");
    assert_eq!(lane("translation").language, None);
}

fn pcm(frames: std::ops::Range<i16>, channels: u16) -> Vec<u8> {
    frames.flat_map(|f| (0..channels).flat_map(move |c| (f * 3 + c as i16).to_le_bytes())).collect()
}

fn pending_dir(dir: &TempDir) -> PathBuf {
    dir.0.join(capture::PENDING_DIR)
}

fn wavs(dir: &TempDir) -> usize {
    fs::read_dir(pending_dir(dir)).map(|d| d.count()).unwrap_or(0)
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

/// A valid WAV: the sizes in the header match the bytes on disk.
fn assert_valid_wav(path: &std::path::Path, frames: usize, block: usize) {
    let bytes = fs::read(path).unwrap();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(bytes.len(), 44 + frames * block);
    assert_eq!(u32_at(&bytes, 4) as usize, 36 + frames * block, "RIFF size");
    assert_eq!(u32_at(&bytes, 40) as usize, frames * block, "data size");
}

fn pending_path(dir: &TempDir, id: &str) -> PathBuf {
    pending_dir(dir).join(format!("{id}.wav"))
}

#[test]
fn recording_lands_as_a_source_with_a_valid_wav() {
    let (dir, mut s) = created("prs_Alice");
    let caps = Captures::default();
    let root = session::project_root(&mut s).unwrap();
    let id = caps.begin(&root, 48_000, 2).unwrap();
    assert!(pending_dir(&dir).is_dir(), "takes live outside cache/");
    assert!(!dir.0.join("cache").join("capture-tmp").exists());
    for i in 0..4 {
        caps.chunk(&id, &pcm(i * 1000..(i + 1) * 1000, 2)).unwrap();
    }
    let done = caps.finish(&id).unwrap();
    assert_valid_wav(&done.path, 4000, 4);
    let bytes = fs::read(&done.path).unwrap();
    assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 2);
    assert_eq!(u32_at(&bytes, 24), 48_000);
    assert_eq!(u32_at(&bytes, 28), 48_000 * 4, "byte rate");
    assert_eq!(&bytes[44..48], &pcm(0..1, 2)[..], "samples are written as sent");

    let row = session::import_capture(&mut s, done, "  Recording 2026-11-09 08:15 ").unwrap();
    assert_eq!(row.label, "Recording 2026-11-09 08:15");
    assert_eq!(row.kind.as_deref(), Some("audio"));
    assert!(row.has_media);
    let info = session::peaks_info(&mut s, &row.id).unwrap();
    assert_eq!((info.frames, info.sample_rate, info.channels), (4000, 48_000, 2));
    assert_eq!(wavs(&dir), 0, "the pending file goes once the import committed");
}

#[test]
fn capture_rejects_bad_input_and_only_discard_deletes() {
    let (dir, mut s) = created("prs_Alice");
    let caps = Captures::default();
    let root = session::project_root(&mut s).unwrap();
    assert!(caps.begin(&root, 48_000, 3).is_err());
    assert!(caps.begin(&root, 0, 1).is_err());

    let id = caps.begin(&root, 44_100, 1).unwrap();
    assert!(caps.chunk(&id, &[0, 0, 0]).is_err(), "half a frame is refused");
    assert!(caps.chunk("nope", &[0, 0]).is_err());
    caps.chunk(&id, &pcm(0..100, 1)).unwrap();
    assert_eq!(wavs(&dir), 1);
    caps.finish(&id).unwrap();
    caps.forget(&id).unwrap();
    assert_eq!(wavs(&dir), 1, "forgetting a capture never touches its file");
    caps.release_all().unwrap();
    assert_eq!(wavs(&dir), 1, "releasing captures never touches their files");

    let id2 = caps.begin(&root, 44_100, 1).unwrap();
    caps.discard(&id2).unwrap();
    assert_eq!(wavs(&dir), 1, "discard removes its own take only");
    caps.discard(&id2).unwrap();
    assert!(caps.chunk(&id2, &[0, 0]).is_err(), "a discarded capture accepts nothing");
    assert!(session::list_sources(&mut s).unwrap().is_empty());
}

/// The updater's guard: refused while a take is open (even one stopped and
/// unsaved), and while it is held no take can begin.
#[test]
fn no_take_begins_while_the_updater_holds_the_capture_table() {
    let (_dir, mut s) = created("prs_Alice");
    let caps = std::sync::Arc::new(Captures::default());
    let root = session::project_root(&mut s).unwrap();
    let id = caps.begin(&root, 8_000, 1).unwrap();
    caps.finish(&id).unwrap();
    assert!(caps.none_open().unwrap().is_none(), "a stopped, unsaved take still blocks");
    caps.discard(&id).unwrap();

    let guard = caps.none_open().unwrap().expect("no take is open");
    let (began, done) = std::sync::mpsc::channel();
    let (c, r) = (caps.clone(), root.clone());
    let t = std::thread::spawn(move || began.send(c.begin(&r, 8_000, 1)).unwrap());
    assert!(done.recv_timeout(std::time::Duration::from_millis(200)).is_err(), "begin waits while the guard is held");
    drop(guard);
    let id = done.recv().unwrap().unwrap();
    t.join().unwrap();
    assert!(caps.none_open().unwrap().is_none());
    caps.discard(&id).unwrap();
}

#[test]
fn nothing_sweeps_old_pending_recordings() {
    let (dir, mut s) = created("prs_Alice");
    let caps = Captures::default();
    let root = session::project_root(&mut s).unwrap();
    let id = caps.begin(&root, 8_000, 1).unwrap();
    caps.chunk(&id, &pcm(0..100, 1)).unwrap();
    let old = pending_path(&dir, &id);
    let stamp = std::time::SystemTime::now() - std::time::Duration::from_secs(30 * 86_400);
    fs::File::options().write(true).open(&old).unwrap().set_modified(stamp).unwrap();
    drop(s);

    let mut reopened: Session = None;
    session::open_project(&mut reopened, dir.path(), "prs_Alice").unwrap();
    assert!(old.exists(), "reopening the project deletes nothing");
    assert_eq!(wavs(&dir), 1);
}

#[test]
fn the_header_is_patched_every_few_seconds_so_a_crash_leaves_a_valid_wav() {
    let (dir, mut s) = created("prs_Alice");
    let caps = Captures::default();
    let root = session::project_root(&mut s).unwrap();
    let id = caps.begin(&root, 8_000, 1).unwrap();
    let path = pending_path(&dir, &id);
    let mut written = 0usize;
    // Four seconds in: under the 5 s interval, so sizes are still zero.
    for _ in 0..4 {
        caps.chunk(&id, &pcm(0..8000, 1)).unwrap();
        written += 8000;
    }
    assert_eq!(u32_at(&fs::read(&path).unwrap(), 40), 0);
    // The chunk that crosses 5 s patches the header to the length so far.
    caps.chunk(&id, &pcm(0..8000, 1)).unwrap();
    written += 8000;
    assert_valid_wav(&path, written, 2);
    // More audio, then "crash": no finish. The header lags by under 5 s.
    caps.chunk(&id, &pcm(0..2000, 1)).unwrap();
    let bytes = fs::read(&path).unwrap();
    assert_eq!(bytes.len(), 44 + (written + 2000) * 2);
    assert_eq!(u32_at(&bytes, 40) as usize, written * 2);
    assert!(2000 < 5 * 8000, "the unsynced tail is under the interval");
}

#[test]
fn a_torn_trailing_frame_is_trimmed_and_the_header_repaired() {
    let (dir, mut s) = created("prs_Alice");
    let caps = Captures::default();
    let root = session::project_root(&mut s).unwrap();
    let id = caps.begin(&root, 8_000, 2).unwrap();
    caps.chunk(&id, &pcm(0..500, 2)).unwrap();
    let path = pending_path(&dir, &id);
    // Simulate a crash: the process is gone (new Captures), and a frame is half written.
    drop(caps);
    let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
    std::io::Write::write_all(&mut f, &[1, 2, 3]).unwrap();
    drop(f);

    let fresh = Captures::default();
    let found = fresh.recover_list(&root).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, id);
    assert_eq!(found[0].duration_ms, 500.0 * 1000.0 / 8000.0);
    assert_valid_wav(&path, 500, 4);
    let bytes = fs::read(&path).unwrap();
    assert_eq!(&bytes[44..48], &pcm(0..1, 2)[..], "audio before the tear is intact");
}

#[test]
fn recovery_lists_leftovers_skips_live_captures_and_never_imports_silently() {
    let (dir, mut s) = created("prs_Alice");
    let caps = Captures::default();
    let root = session::project_root(&mut s).unwrap();
    let live = caps.begin(&root, 8_000, 1).unwrap();
    caps.chunk(&live, &pcm(0..100, 1)).unwrap();
    let left = caps.begin(&root, 8_000, 1).unwrap();
    caps.chunk(&left, &pcm(0..200, 1)).unwrap();
    caps.finish(&left).unwrap();
    // Something that is not ours stays out of the list and on disk.
    let stray = pending_dir(&dir).join("notes.wav");
    fs::write(&stray, b"hello").unwrap();

    // `left` is still tracked (finished, unsaved) and `live` is recording: both are held.
    assert!(caps.recover_list(&root).unwrap().is_empty(), "live and unsaved takes are not recovered");
    assert!(caps.recover_take(&root, &live).is_err(), "a live take cannot be imported as recovered");
    assert!(caps.discard_pending(&root, &live).is_err(), "nor discarded");

    // The page went away: the captures are released and now show up.
    assert_eq!(caps.release_all().unwrap(), 2);
    let ids: Vec<String> = caps.recover_list(&root).unwrap().into_iter().map(|p| p.id).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&live) && ids.contains(&left));
    assert!(stray.exists());
    let empty = caps.begin(&root, 8_000, 1).unwrap();
    caps.release_all().unwrap();
    assert!(!caps.recover_list(&root).unwrap().iter().any(|p| p.id == empty), "a take with no audio has nothing to recover");
    assert!(pending_path(&dir, &empty).exists(), "and is not deleted");
    assert!(session::list_sources(&mut s).unwrap().is_empty(), "listing never imports");
    assert_eq!(wavs(&dir), 4);
    assert!(caps.recover_take(&root, "../x").is_err(), "ids are not paths");
}

#[test]
fn importing_a_recovered_recording_removes_the_file_only_after_success() {
    let (dir, mut s) = created("prs_Alice");
    let caps = Captures::default();
    let root = session::project_root(&mut s).unwrap();
    let id = caps.begin(&root, 8_000, 1).unwrap();
    caps.chunk(&id, &pcm(0..300, 1)).unwrap();
    caps.release_all().unwrap();
    let path = pending_path(&dir, &id);

    // Break the blob store: the import fails and the file must stay.
    let blobs = dir.0.join("blobs");
    fs::remove_dir_all(&blobs).ok();
    fs::write(&blobs, b"not a directory").unwrap();
    let done = caps.recover_take(&root, &id).unwrap();
    assert!(session::import_capture(&mut s, done, "Take").is_err());
    assert!(path.exists(), "failed import keeps the file");
    assert!(session::list_sources(&mut s).unwrap().is_empty());

    fs::remove_file(&blobs).unwrap();
    let done = caps.recover_take(&root, &id).unwrap();
    let row = session::import_capture(&mut s, done, "Take").unwrap();
    assert_eq!(session::peaks_info(&mut s, &row.id).unwrap().frames, 300);
    assert!(!path.exists(), "removed once the intents committed");
}

#[test]
fn a_failed_import_keeps_the_take_and_a_second_finish_retries_it() {
    let (dir, mut s) = created("prs_Alice");
    let caps = Captures::default();
    let root = session::project_root(&mut s).unwrap();
    let id = caps.begin(&root, 8_000, 1).unwrap();
    caps.chunk(&id, &pcm(0..300, 1)).unwrap();
    let path = pending_path(&dir, &id);

    let blobs = dir.0.join("blobs");
    fs::remove_dir_all(&blobs).ok();
    fs::write(&blobs, b"not a directory").unwrap();
    let done = caps.finish(&id).unwrap();
    assert!(session::import_capture(&mut s, done, "Take").is_err());
    assert!(path.exists());

    fs::remove_file(&blobs).unwrap();
    let again = caps.finish(&id).expect("finish is idempotent");
    // The take's root was canonicalized at begin (/private/var on macOS, \\?\ on Windows).
    assert_eq!(fs::canonicalize(&again.path).unwrap(), fs::canonicalize(&path).unwrap());
    assert!(caps.chunk(&id, &[0, 0]).is_err(), "a finished take takes no more audio");
    let row = session::import_capture(&mut s, again, "Take").unwrap();
    assert!(row.has_media);
    caps.forget(&id).unwrap();
    assert!(!path.exists());
    assert!(caps.finish(&id).is_err(), "after success the capture is gone");
}

#[test]
fn a_take_finished_after_the_project_changed_stays_pending() {
    let (dir_a, mut s) = created("prs_Alice");
    let caps = Captures::default();
    let root_a = session::project_root(&mut s).unwrap();
    let id = caps.begin(&root_a, 8_000, 1).unwrap();
    caps.chunk(&id, &pcm(0..300, 1)).unwrap();

    // Another project becomes the open one while the take is running.
    let dir_b = TempDir::new();
    session::create_project(&mut s, dir_b.path(), "prs_Alice").unwrap();
    let done = caps.finish(&id).unwrap();
    let err = session::import_capture(&mut s, done, "Take").unwrap_err();
    assert!(err.contains("the project changed during the recording; it is kept at"), "{err}");
    assert!(session::list_sources(&mut s).unwrap().is_empty(), "nothing leaks into project B");
    assert_eq!(wavs(&dir_a), 1, "the file stays in the project it started in");
    assert_eq!(wavs(&dir_b), 0);

    // After the page is gone and project A reopens, it is a Recovered recording.
    caps.release_all().unwrap();
    session::open_project(&mut s, dir_a.path(), "prs_Alice").unwrap();
    let root = session::project_root(&mut s).unwrap();
    let found = caps.recover_list(&root).unwrap();
    assert_eq!(found.len(), 1);
    let done = caps.recover_take(&root, &found[0].id).unwrap();
    session::import_capture(&mut s, done, "Take").unwrap();
    assert_eq!(wavs(&dir_a), 0);
}

#[test]
fn keeping_a_take_for_later_leaves_a_recovered_recording() {
    let (dir, mut s) = created("prs_Alice");
    let caps = Captures::default();
    let root = session::project_root(&mut s).unwrap();
    let id = caps.begin(&root, 8_000, 1).unwrap();
    caps.chunk(&id, &pcm(0..300, 1)).unwrap();
    caps.release(&id).unwrap();
    assert_valid_wav(&pending_path(&dir, &id), 300, 2);
    assert_eq!(caps.recover_list(&root).unwrap().len(), 1);
    assert!(caps.chunk(&id, &[0, 0]).is_err(), "a released take is no longer held");
}

#[test]
fn a_closed_project_keeps_the_file_too() {
    let (dir, mut s) = created("prs_Alice");
    let caps = Captures::default();
    let root = session::project_root(&mut s).unwrap();
    let id = caps.begin(&root, 8_000, 1).unwrap();
    caps.chunk(&id, &pcm(0..300, 1)).unwrap();
    let done = caps.finish(&id).unwrap();
    let mut closed: Session = None;
    let err = session::import_capture(&mut closed, done, "Take").unwrap_err();
    assert!(err.contains("the recording is kept at"), "{err}");
    assert_eq!(wavs(&dir), 1);
}

#[test]
fn the_size_cap_keeps_everything_and_leaves_a_whole_frame_wav() {
    let (dir, mut s) = created("prs_Alice");
    let caps = Captures::with_limit(1000);
    let root = session::project_root(&mut s).unwrap();
    let id = caps.begin(&root, 8_000, 2).unwrap();
    caps.chunk(&id, &pcm(0..200, 2)).unwrap();
    let err = caps.chunk(&id, &pcm(0..100, 2)).unwrap_err();
    assert!(err.contains("4 GiB"), "{err}");
    let done = caps.finish(&id).unwrap();
    assert_valid_wav(&done.path, 250, 4);
    assert_eq!(wavs(&dir), 1, "the take is kept");
    assert!(caps.chunk(&id, &pcm(0..1, 2)).is_err());
}

#[test]
fn only_the_apps_own_origin_may_use_the_microphone() {
    let url = |u: &str| u.parse::<tauri::Url>().unwrap();
    let dev = url("http://localhost:1420");
    for ok in ["tauri://localhost/", "tauri://localhost/index.html?x=1", "http://tauri.localhost/", "https://tauri.localhost/a"] {
        assert!(capture::is_app_origin(&url(ok), None), "{ok}");
    }
    for bad in ["https://example.com/", "http://tauri.localhost.evil.com/", "tauri://evil/", "http://localhost:1420/", "file:///tmp/x.html", "about:blank"] {
        assert!(!capture::is_app_origin(&url(bad), None), "{bad}");
    }
    assert!(capture::is_app_origin(&url("http://localhost:1420/x"), Some(&dev)));
    assert!(!capture::is_app_origin(&url("http://localhost:1421/"), Some(&dev)));
    assert!(!capture::is_app_origin(&url("http://127.0.0.1:1420/"), Some(&dev)));
}
