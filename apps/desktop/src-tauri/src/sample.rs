//! The tutorial project (roadmap 68): a short recording already segmented,
//! transcribed, translated, glossed and in the lexicon, so a first-time user
//! sees what every panel does before bringing data of their own.
//!
//! The recording is the maintainer's own voice counting from one to six in
//! English, published by him under CC BY 4.0 (`tutorial/README.md`); it is
//! the one piece of real speech in the repository. The project is written
//! through the same intents as any edit, so it is an ordinary project the
//! person can change, break or delete.

use std::path::Path;

use crate::gloss::MorphSpecView;
use crate::session::{self, project, ProjectInfo, Session};

/// The recording, 16 kHz mono PCM16.
const RECORDING: &[u8] = include_bytes!("../tutorial/counting-to-six.wav");

/// One word: its IPA transcription, its gloss, and where it is spoken
/// (seconds, measured from the recording with a little room either side).
struct Word {
    form: &'static str,
    gloss: &'static str,
    span: (f64, f64),
}

/// One utterance: its words, the free translation, and whether the tutorial
/// leaves it untranscribed (so the person has one to try).
struct Utterance {
    words: &'static [Word],
    translation: &'static str,
    transcribed: bool,
}

const UTTERANCES: &[Utterance] = &[
    Utterance {
        words: &[
            Word { form: "wʌn", gloss: "one", span: (0.66, 1.34) },
            Word { form: "tuː", gloss: "two", span: (1.38, 2.06) },
            Word { form: "θɹiː", gloss: "three", span: (2.12, 2.72) },
        ],
        translation: "One, two, three.",
        transcribed: true,
    },
    Utterance {
        words: &[Word { form: "fɔɹ", gloss: "four", span: (2.84, 3.50) }, Word { form: "faɪv", gloss: "five", span: (3.58, 4.22) }],
        translation: "Four, five.",
        transcribed: true,
    },
    Utterance { words: &[Word { form: "sɪks", gloss: "six", span: (4.30, 5.12) }], translation: "", transcribed: false },
];

/// Each utterance's span in seconds: from its first word to its last.
fn span(u: &Utterance) -> (f64, f64) {
    (u.words[0].span.0, u.words[u.words.len() - 1].span.1)
}

pub const TITLE: &str = "Counting to six (tutorial)";

/// Written last, when the sample is whole. A folder without it is a build
/// that did not finish (a full disk, a crash), so it is wiped and built again;
/// the folder is the app's own, in app data, where nobody else writes.
const COMPLETE: &str = "sample-complete";

/// Open the sample project at `dir`, building it first when it is not there.
pub fn open_sample(session: &mut Session, dir: &Path, researcher: &str) -> Result<ProjectInfo, String> {
    let path = dir.to_string_lossy().to_string();
    if dir.join(COMPLETE).is_file() && dir.join("project.json").is_file() {
        let info = session::open_project(session, &path, researcher)?;
        link_older_sample(session)?;
        return Ok(info);
    }
    // Close whatever is open first: a half-built sample may be the open project.
    *session = None;
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| format!("could not clear an unfinished tutorial: {e}"))?;
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("could not make the tutorial folder: {e}"))?;
    let built = session::create_project(session, &path, researcher).and_then(|_| build(session, dir));
    if let Err(e) = built {
        // Nothing half-built stays open or on disk; the next click starts over.
        *session = None;
        let _ = std::fs::remove_dir_all(dir);
        return Err(format!("the tutorial project could not be made: {e}"));
    }
    std::fs::write(dir.join(COMPLETE), "1\n").map_err(|e| e.to_string())?;
    session::open_project(session, &path, researcher)
}

/// A sample built before texts remembered their recording (roadmap 72) has
/// one text and one recording, unlinked: link them, once.
fn link_older_sample(session: &mut Session) -> Result<(), String> {
    let texts = session::list_texts(session)?;
    let sources = session::list_sources(session)?;
    if let ([text], [source]) = (texts.as_slice(), sources.as_slice()) {
        if text.recording.is_none() && source.has_media {
            session::link_recording(session, &text.id, Some(&source.id))?;
        }
    }
    Ok(())
}

fn build(session: &mut Session, dir: &Path) -> Result<(), String> {
    // The recording, through the ordinary import.
    let wav = dir.join("cache").join("tutorial-recording.wav");
    std::fs::create_dir_all(wav.parent().unwrap_or(dir)).map_err(|e| e.to_string())?;
    std::fs::write(&wav, RECORDING).map_err(|e| e.to_string())?;
    let imported = session::import_media_named(session, &wav, "Counting to six", None);
    std::fs::remove_file(&wav).ok();
    let recording = imported?;

    let text = session::create_text(session, TITLE)?;
    session::link_recording(session, &text, Some(&recording.id))?;
    let translation = session::add_tier(session, "translation", None)?.id;

    // The lexicon first, so each glossed morph links to its sense.
    let mut senses = std::collections::BTreeMap::new();
    for u in UTTERANCES {
        for w in u.words {
            if !senses.contains_key(w.form) {
                let (_, sense) = project(session)?.create_lexeme(w.form, w.gloss)?;
                senses.insert(w.form, sense);
            }
        }
    }

    for u in UTTERANCES {
        let (start, end) = span(u);
        let (s0, s1) = (start * 1000.0, end * 1000.0);
        let content = if u.transcribed { u.words.iter().map(|w| w.form).collect::<Vec<_>>().join(" ") } else { String::new() };
        let seg = session::add_segment(session, &text, None, Some(s0.round()), Some(s1.round()), &content)?.id;
        if !u.translation.is_empty() {
            session::add_segment(session, &text, Some(&translation), Some(s0.round()), Some(s1.round()), u.translation)?;
        }
        if !u.transcribed {
            continue;
        }
        let forms: Vec<String> = u.words.iter().map(|w| w.form.to_string()).collect();
        let view = crate::gloss::gloss_tokenize(session, &text, &seg, &forms)?.view;
        for (word, w) in view.words.iter().zip(u.words) {
            let morphs = vec![MorphSpecView {
                surface: w.form.to_string(),
                morph_type: "root".to_string(),
                gloss: Some(w.gloss.to_string()),
                glosses_by_lang: Default::default(),
                sense: senses.get(w.form).cloned(),
            }];
            crate::gloss::gloss_apply(session, &text, &seg, &word.id, morphs)?;
        }
    }
    Ok(())
}

/// For the corpus tools' tests: one more utterance with a prefix and a
/// clitic (`ni-kani teka=ma`, "I count to ten"), its root linked to a
/// lexicon entry and a translation, since the tutorial's English has no
/// morphology to find.
#[cfg(test)]
pub fn add_morphology(session: &mut Session) -> Result<(), String> {
    let text = session::list_texts(session)?[0].id.clone();
    let tiers = session::get_text(session, &text)?.tiers;
    let translation = tiers.iter().find(|t| t.name == "translation").map(|t| t.id.clone()).ok_or("no translation tier")?;
    let (_, count) = project(session)?.create_lexeme("kani", "count")?;
    let seg = session::add_segment(session, &text, None, Some(6_000.0), Some(7_000.0), "nikani tekama")?.id;
    session::add_segment(session, &text, Some(&translation), Some(6_000.0), Some(7_000.0), "I count to ten.")?;
    let forms = vec!["nikani".to_string(), "tekama".to_string()];
    let view = crate::gloss::gloss_tokenize(session, &text, &seg, &forms)?.view;
    let m = |surface: &str, kind: &str, gloss: &str, sense: Option<String>| MorphSpecView { surface: surface.into(), morph_type: kind.into(), gloss: Some(gloss.into()), glosses_by_lang: Default::default(), sense };
    crate::gloss::gloss_apply(session, &text, &seg, &view.words[0].id, vec![m("ni", "prefix", "1SG", None), m("kani", "root", "count", Some(count))])?;
    crate::gloss::gloss_apply(session, &text, &seg, &view.words[1].id, vec![m("teka", "root", "ten", None), m("ma", "clitic", "FOC", None)])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_recording_is_loud_where_the_words_are() {
        assert_eq!(&RECORDING[0..4], b"RIFF");
        assert_eq!(&RECORDING[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(RECORDING[24..28].try_into().unwrap()), 16_000);
        // The audio starts after the `data` chunk's header, wherever ffmpeg put it.
        let data = RECORDING.windows(4).position(|w| w == b"data").expect("a data chunk") + 8;
        let samples: Vec<f64> = RECORDING[data..].chunks_exact(2).map(|b| f64::from(i16::from_le_bytes([b[0], b[1]]))).collect();
        let rms = |a: f64, b: f64| {
            let (i, j) = ((a * 16_000.0) as usize, ((b * 16_000.0) as usize).min(samples.len()));
            (samples[i..j].iter().map(|s| s * s).sum::<f64>() / (j - i) as f64).sqrt()
        };
        let quiet = rms(0.05, 0.6);
        let mut last = 0.0;
        for u in UTTERANCES {
            for w in u.words {
                assert!(w.span.0 >= last && w.span.1 > w.span.0, "{} is in order", w.form);
                last = w.span.1;
                let (a, b) = w.span;
                assert!(rms(a + (b - a) * 0.3, a + (b - a) * 0.6) > 10.0 * quiet, "{} is spoken where its segment is", w.form);
            }
        }
    }

    #[test]
    fn the_sample_builds_and_reopens() {
        let dir = std::env::temp_dir().join(format!("wugs-sample-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        let info = open_sample(&mut session, &dir, "tester").expect("sample builds");
        assert_eq!(info.texts, 1);
        let texts = session::list_texts(&mut session).unwrap();
        let detail = session::get_text(&mut session, &texts[0].id).unwrap();
        assert_eq!(detail.title, TITLE);
        // Three utterances, two translations; the last utterance is left to transcribe.
        assert_eq!(detail.segments.iter().filter(|s| detail.tiers.iter().any(|t| t.id == s.tier && t.utterance)).count(), 3);
        assert!(detail.segments.iter().any(|s| s.content.is_empty()));
        let sources = session::list_sources(&mut session).unwrap();
        assert_eq!(sources.len(), 1);
        // The text knows its recording, so every panel can play it.
        assert_eq!(texts[0].recording.as_deref(), Some(sources[0].id.as_str()));
        // Picking the same recording again writes nothing; forgetting it clears the link.
        session::link_recording(&mut session, &texts[0].id, Some(&sources[0].id)).unwrap();
        session::link_recording(&mut session, &texts[0].id, None).unwrap();
        assert_eq!(session::list_texts(&mut session).unwrap()[0].recording, None);
        // A sample from before links were kept is linked when it is opened again.
        let again = open_sample(&mut session, &dir, "tester").unwrap();
        assert_eq!(again.texts, 1);
        assert_eq!(session::list_texts(&mut session).unwrap()[0].recording.as_deref(), Some(sources[0].id.as_str()));
        // Opening it again reuses it rather than building a second copy.
        let again = open_sample(&mut session, &dir, "tester").unwrap();
        assert_eq!(again.texts, 1);
        // An unfinished build (no completion mark) is wiped and made again.
        drop(session);
        std::fs::remove_file(dir.join(COMPLETE)).unwrap();
        let mut session: Session = None;
        let rebuilt = open_sample(&mut session, &dir, "tester").unwrap();
        assert_eq!(rebuilt.texts, 1);
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
