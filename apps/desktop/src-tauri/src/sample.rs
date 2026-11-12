//! The sample project (roadmap 68): a short recording already segmented,
//! transcribed, translated, glossed and in the lexicon, so a first-time user
//! sees what every panel does before bringing data of their own.
//!
//! Everything in it is made up here. The "language" is invented and the
//! recording is synthesized (voiced syllables built from harmonics, a quiet
//! noise floor), never real speech: the repository is public and holds no
//! field data. It is written through the same intents as any edit, so it
//! is an ordinary project the person can change, break or delete.

use std::f64::consts::PI;
use std::path::Path;

use crate::gloss::MorphSpecView;
use crate::session::{self, project, ProjectInfo, Session};

const RATE: u32 = 16_000;
/// Where the first syllable starts, and the gaps between words and utterances.
const LEAD_S: f64 = 0.6;
const WORD_GAP_S: f64 = 0.14;
const UTTERANCE_GAP_S: f64 = 0.9;
/// Padding kept around each utterance's words when its segment is drawn.
const PAD_S: f64 = 0.08;

/// One word: its spelling, its morphs (surface, type, gloss) and how long it is spoken.
struct Word {
    form: &'static str,
    morphs: &'static [(&'static str, &'static str, &'static str)],
}

/// One utterance: its words, the free translation, and whether the sample
/// leaves it untranscribed (so the person has one to try).
struct Utterance {
    words: &'static [Word],
    translation: &'static str,
    transcribed: bool,
}

const UTTERANCES: &[Utterance] = &[
    Utterance {
        words: &[
            Word { form: "nikani", morphs: &[("ni", "prefix", "1SG"), ("kani", "root", "count")] },
            Word { form: "ŋai", morphs: &[("ŋai", "root", "one")] },
            Word { form: "toɾu", morphs: &[("toɾu", "root", "two")] },
            Word { form: "kesi", morphs: &[("kesi", "root", "three")] },
        ],
        translation: "I count: one, two, three.",
        transcribed: true,
    },
    Utterance {
        words: &[
            Word { form: "mopa", morphs: &[("mopa", "root", "four")] },
            Word { form: "liwa", morphs: &[("liwa", "root", "five")] },
            Word { form: "senu", morphs: &[("senu", "root", "six")] },
        ],
        translation: "Four, five, six.",
        transcribed: true,
    },
    Utterance {
        words: &[
            Word { form: "paki", morphs: &[("paki", "root", "seven")] },
            Word { form: "ɾata", morphs: &[("ɾata", "root", "eight")] },
            Word { form: "wuno", morphs: &[("wuno", "root", "nine")] },
            Word { form: "tekama", morphs: &[("teka", "root", "ten"), ("ma", "clitic", "FOC")] },
        ],
        translation: "Seven, eight, nine, and ten.",
        transcribed: true,
    },
    Utterance {
        words: &[
            Word { form: "nikaniɾa", morphs: &[] },
            Word { form: "teka", morphs: &[] },
        ],
        translation: "",
        transcribed: false,
    },
];

/// Seconds a word lasts: a syllable is about 0.19 s.
fn word_seconds(w: &Word) -> f64 {
    let vowels = w.form.chars().filter(|c| "aeiou".contains(*c)).count().max(1);
    0.19 * vowels as f64
}

/// Each utterance's span in seconds, and each word's within it, in order.
fn timing() -> Vec<(f64, f64, Vec<(f64, f64)>)> {
    let mut t = LEAD_S;
    let mut out = Vec::new();
    for u in UTTERANCES {
        let start = t;
        let mut words = Vec::new();
        for (i, w) in u.words.iter().enumerate() {
            if i > 0 {
                t += WORD_GAP_S;
            }
            let d = word_seconds(w);
            words.push((t, t + d));
            t += d;
        }
        out.push((start, t, words));
        t += UTTERANCE_GAP_S;
    }
    out
}

/// A small deterministic noise source, so the sample is the same every time.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f64 / (1u64 << 31) as f64) * 2.0 - 1.0
    }
}

/// The synthetic recording: each syllable a short consonant hiss then a
/// voiced vowel (harmonics of a falling pitch, shaped to sound vowel-like),
/// over a faint noise floor, as 16 kHz mono PCM16.
pub fn synth_samples() -> Vec<i16> {
    let spans = timing();
    let total = spans.last().map(|s| s.1).unwrap_or(0.0) + 0.8;
    let n = (total * f64::from(RATE)) as usize;
    let mut out = vec![0f64; n];
    let mut noise = Lcg(0x5eed);
    for x in out.iter_mut() {
        *x = noise.next() * 0.0015;
    }
    for (ui, (_, _, words)) in spans.iter().enumerate() {
        for (wi, &(ws, we)) in words.iter().enumerate() {
            let syllables = ((we - ws) / 0.19).round().max(1.0) as usize;
            let syl = (we - ws) / syllables as f64;
            for k in 0..syllables {
                let s0 = ws + k as f64 * syl;
                let consonant = 0.045;
                // Vowel colour cycles through a, i, u, e, o so words differ.
                let vowel = (ui * 7 + wi * 3 + k) % 5;
                let formants: [(f64, f64); 5] = [(750.0, 1200.0), (300.0, 2300.0), (320.0, 800.0), (480.0, 1900.0), (500.0, 900.0)];
                let (f1, f2) = formants[vowel];
                let a = (s0 * f64::from(RATE)) as usize;
                let b = ((s0 + syl) * f64::from(RATE)).min(n as f64) as usize;
                for (i, x) in out.iter_mut().enumerate().take(b).skip(a) {
                    let t = (i - a) as f64 / f64::from(RATE);
                    let len = syl;
                    if t < consonant {
                        *x += noise.next() * 0.05 * (t / consonant);
                        continue;
                    }
                    let tv = t - consonant;
                    let dv = len - consonant;
                    let env = (tv / 0.03).min(1.0) * ((dv - tv) / 0.05).clamp(0.0, 1.0);
                    let f0 = 150.0 - 30.0 * (tv / dv) - 6.0 * ui as f64;
                    let phase = 2.0 * PI * f0 * tv;
                    let mut v = 0.0;
                    for h in 1..=24 {
                        let f = f0 * h as f64;
                        let gain = 1.0 / h as f64 * (1.0 + 3.0 * (-((f - f1) / 120.0).powi(2)).exp() + 2.0 * (-((f - f2) / 180.0).powi(2)).exp());
                        v += gain * (phase * h as f64).sin();
                    }
                    *x += 0.12 * env * v;
                }
            }
        }
    }
    let peak = out.iter().fold(0.0f64, |m, x| m.max(x.abs())).max(1e-9);
    out.iter().map(|x| ((x / peak) * 0.7 * f64::from(i16::MAX)) as i16).collect()
}

/// The recording as a WAV file's bytes.
pub fn synth_wav() -> Vec<u8> {
    let samples = synth_samples();
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

pub const TITLE: &str = "Counting to ten (sample)";

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
        std::fs::remove_dir_all(dir).map_err(|e| format!("could not clear an unfinished sample: {e}"))?;
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("could not make the sample folder: {e}"))?;
    let built = session::create_project(session, &path, researcher).and_then(|_| build(session, dir));
    if let Err(e) = built {
        // Nothing half-built stays open or on disk; the next click starts over.
        *session = None;
        let _ = std::fs::remove_dir_all(dir);
        return Err(format!("the sample project could not be made: {e}"));
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
    let wav = dir.join("cache").join("sample-recording.wav");
    std::fs::create_dir_all(wav.parent().unwrap_or(dir)).map_err(|e| e.to_string())?;
    std::fs::write(&wav, synth_wav()).map_err(|e| e.to_string())?;
    let imported = session::import_media_named(session, &wav, "Counting to ten (synthetic voice)", None);
    std::fs::remove_file(&wav).ok();
    let recording = imported?;

    let text = session::create_text(session, TITLE)?;
    session::link_recording(session, &text, Some(&recording.id))?;
    let translation = session::add_tier(session, "translation", None)?.id;

    // The lexicon first, so each glossed morph links to its sense.
    let mut senses = std::collections::BTreeMap::new();
    for u in UTTERANCES {
        for w in u.words {
            for &(surface, kind, gloss) in w.morphs {
                if kind != "root" || senses.contains_key(surface) {
                    continue;
                }
                let (_, sense) = project(session)?.create_lexeme(surface, gloss)?;
                senses.insert(surface, sense);
            }
        }
    }

    for (u, (start, end, _)) in UTTERANCES.iter().zip(timing()) {
        let (s0, s1) = ((start - PAD_S) * 1000.0, (end + PAD_S) * 1000.0);
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
            if w.morphs.is_empty() {
                continue;
            }
            let morphs = w
                .morphs
                .iter()
                .map(|&(surface, kind, gloss)| MorphSpecView {
                    surface: surface.to_string(),
                    morph_type: kind.to_string(),
                    gloss: Some(gloss.to_string()),
                    glosses_by_lang: Default::default(),
                    sense: if kind == "root" { senses.get(surface).cloned() } else { None },
                })
                .collect();
            crate::gloss::gloss_apply(session, &text, &seg, &word.id, morphs)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_recording_is_a_valid_wav_with_speech_where_the_segments_are() {
        let wav = synth_wav();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        let samples = synth_samples();
        let rms = |a: f64, b: f64| {
            let (i, j) = ((a * f64::from(RATE)) as usize, (b * f64::from(RATE)) as usize);
            (samples[i..j].iter().map(|&s| f64::from(s).powi(2)).sum::<f64>() / (j - i) as f64).sqrt()
        };
        let spans = timing();
        let (start, end, words) = &spans[0];
        // Loud inside a word, near silent between utterances.
        let (w0, w1) = words[1];
        assert!(rms(w0 + 0.05, w1 - 0.05) > 20.0 * rms(end + 0.2, end + 0.6));
        assert!(*start >= LEAD_S && end > start);
        assert_eq!(spans.len(), UTTERANCES.len());
    }

    #[test]
    fn every_glossed_word_spells_its_morphs() {
        for u in UTTERANCES {
            for w in u.words {
                if w.morphs.is_empty() {
                    continue;
                }
                let joined: String = w.morphs.iter().map(|m| m.0).collect();
                assert_eq!(joined, w.form, "{} must be the concatenation of its morphs", w.form);
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
        // Four utterances, three translations; the last utterance is left to transcribe.
        assert_eq!(detail.segments.iter().filter(|s| detail.tiers.iter().any(|t| t.id == s.tier && t.utterance)).count(), 4);
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
