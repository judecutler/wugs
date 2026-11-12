//! The demo path end to end through the desktop session (docs/DEMO.md): a
//! voice memo in, segmented, transcribed, glossed, in the lexicon, with a
//! consent record, then out as ELAN, LIFT and a sealed package, and the ELAN
//! file back into a fresh project. Each step is tested on its own elsewhere;
//! this checks that they still fit together.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use wugs_asr::vad::Granularity;
use wugs_desktop::capability::Provider;
use wugs_desktop::gloss::{self, MorphSpecView};
use wugs_desktop::interchange::{self, Format};
use wugs_desktop::session::{self, Session};
use wugs_desktop::{convert, lexicon, people, segmentation};
use wugs_model::ulid;

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn morph(surface: &str, kind: &str, gloss: &str) -> MorphSpecView {
    MorphSpecView { surface: surface.into(), morph_type: kind.into(), gloss: Some(gloss.into()), glosses_by_lang: BTreeMap::new(), sense: None }
}

#[test]
fn record_to_archive() {
    let dir = TempDir(std::env::temp_dir().join(format!("wugs-demo-path-{}", ulid())));
    let out = dir.0.join("out");
    fs::create_dir_all(&out).unwrap();
    let mut s: Session = None;
    session::create_project(&mut s, dir.0.join("project").to_str().unwrap(), "prs_Demo").unwrap();

    // 1. A text, and a voice memo decoded and imported.
    let text = session::create_text(&mut s, "Counting").unwrap();
    let memo = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio/two-bursts.m4a");
    let wav = dir.0.join("memo.wav");
    convert::to_wav(&memo, &wav).unwrap();
    let source = session::import_media_named(&mut s, &wav, "two-bursts.m4a", None).unwrap().id;

    // 2. Auto-segment by utterance: two bursts of sound, two segments.
    let job = segmentation::job(&mut s, &source).unwrap();
    let spans = segmentation::spans(&Provider::VoiceActivity, &job, Granularity::Utterances).unwrap();
    assert_eq!(spans.len(), 2, "{spans:?}");
    let made = segmentation::commit(&mut s, &text, None, &spans).unwrap();
    assert_eq!(made.ids.len(), 2);
    let first = made.ids[0].clone();
    assert!((spans[0].0 - 500.0).abs() < 200.0 && (spans[1].1 - 2900.0).abs() < 250.0, "{spans:?}");

    // 3. Transcribe.
    session::edit_segment_content(&mut s, &text, &first, "bahayan isa").unwrap();

    // 4. Gloss: words, a breakdown, a root added to the lexicon.
    let edit = gloss::gloss_tokenize(&mut s, &text, &first, &["bahayan".into(), "isa".into()]).unwrap();
    let word = edit.view.words[0].id.clone();
    let edit = gloss::gloss_apply(&mut s, &text, &first, &word, vec![morph("bahay", "root", "house"), morph("an", "suffix", "LOC")]).unwrap();
    let root = edit.view.words[0].morphs[0].id.clone();
    gloss::gloss_add_lexeme(&mut s, &text, &first, &root, "bahay", "house").unwrap();
    assert!(lexicon::lexicon_list(&mut s, "bahay").unwrap().iter().any(|e| e.lemma == "bahay"));

    // 5. The speaker and what they agreed to.
    let v = people::add_person(&mut s, "Speaker A", "speaker").unwrap();
    let project_id = v.coverable[0].id.clone();
    let v = people::add_consent(&mut s, &v.people[0].id, &project_id, "2026-11-11").unwrap();
    people::set_term(&mut s, &v.people[0].consents[0].id, "archive-ok", Some(true)).unwrap();

    // 6. Out: ELAN, LIFT, an archive package.
    let eaf = interchange::export_text(&mut s, &text, Format::Elan, &out).unwrap();
    let lift = interchange::export_lexicon(&mut s, Format::Lift, &out).unwrap();
    let sealed = interchange::seal_project(&mut s, &out, "2026-11-11").unwrap();
    assert_eq!((sealed.texts, sealed.lexemes), (1, 1));
    assert!(fs::read_to_string(&eaf.path).unwrap().contains("bahayan isa"));
    assert!(fs::read_to_string(&lift.path).unwrap().contains("bahay"));
    assert!(Path::new(&sealed.path).join("bagit.txt").is_file());

    // 7. And back in, somewhere else, with the times kept.
    let mut other: Session = None;
    session::create_project(&mut other, dir.0.join("other").to_str().unwrap(), "prs_Other").unwrap();
    let imported = interchange::import_file(&mut other, &eaf.path).unwrap();
    let back = session::get_text(&mut other, &imported.text.unwrap()).unwrap();
    let seg = back.segments.iter().find(|r| r.content == "bahayan isa").expect("the transcription came back");
    assert_eq!(seg.start_ms.map(f64::round), Some(spans[0].0.round()));
}
