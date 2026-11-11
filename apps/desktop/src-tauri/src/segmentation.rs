//! Segmentation (roadmap 59): where speech starts and ends in a recording,
//! from whatever powers the `segmentation` capability (built-in voice
//! activity today). The Timeline uses the spans twice: Auto-segment writes
//! one segment per span the person has not already covered, and edge
//! snapping pulls a drawn or dragged edge out to the nearest speech edge so
//! a word's onset is not cut off.
//!
//! Spans are computed once per recording and settings, off the session
//! thread, and cached as `<project>/cache/speech-spans/<blob hex>/<key>.json`
//! (a JSON array of `[start_ms, end_ms]`), written whole so a crash never
//! leaves half a file. The cache is disposable like the rest of `cache/`.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use wugs_asr::vad;

use crate::capability::Provider;
use crate::praat::write_atomic;
use crate::session::{self, finding_lines, SegmentStateView, Session};

/// One computation at a time: the snap prefetch and an Auto-segment on the
/// same recording must not both read an hours-long file; the second waits
/// and finds the cache.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Where a recording's audio and span cache are, found on the session thread.
pub struct Job {
    wav: PathBuf,
    dir: PathBuf,
}

pub fn job(session: &mut Session, source: &str) -> Result<Job, String> {
    let p = session::project(session)?;
    let blob = session::audio_blob(p, source)?;
    let hex = wugs_blob::parse_ref(&blob).ok_or_else(|| format!("malformed blob ref {blob:?}"))?.to_string();
    let wav = p.root.join("blobs").join(&hex);
    if !wav.is_file() {
        return Err("The recording's audio file is missing from this project folder.".into());
    }
    Ok(Job { wav, dir: p.root.join("cache").join("speech-spans").join(hex) })
}

fn parse_cache(bytes: &[u8]) -> Option<Vec<(f64, f64)>> {
    let v: Vec<[f64; 2]> = serde_json::from_slice(bytes).ok()?;
    Some(v.into_iter().map(|[s, e]| (s, e)).collect())
}

/// The speech spans of the job's recording, from the cache or computed by
/// `provider` (and cached). Call off the session thread.
pub fn spans(provider: &Provider, job: &Job) -> Result<Vec<(f64, f64)>, String> {
    let Provider::VoiceActivity = provider else {
        return Err(format!("{} cannot segment a recording in this version of wugs.", crate::capability::provider_name(provider.id())));
    };
    let params = vad::Params::default();
    let file = job.dir.join(format!("{}-{}.json", provider.id(), params.key()));
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(hit) = fs::read(&file).ok().and_then(|b| parse_cache(&b)) {
        return Ok(hit);
    }
    let found = vad::speech_spans(&job.wav, params).map_err(|e| format!("Could not read the recording: {e}"))?;
    fs::create_dir_all(&job.dir).map_err(|e| format!("create {}: {e}", job.dir.display()))?;
    let rows: Vec<[f64; 2]> = found.iter().map(|&(s, e)| [s.round(), e.round()]).collect();
    // A cache that cannot be written only costs the next computation.
    write_atomic(&file, &serde_json::to_vec(&rows).expect("serializable")).ok();
    Ok(rows.into_iter().map(|[s, e]| (s, e)).collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoSegmented {
    /// The new segments, in time order, so the shell can undo them.
    pub ids: Vec<String>,
    /// Spans the person had already covered on that tier, left alone.
    pub skipped: usize,
    /// Which tier they went on (the utterance tier when none was given).
    pub tier: String,
    pub findings: Vec<String>,
    /// The new segments as written, for the undo's check.
    pub states: Vec<SegmentStateView>,
}

/// Write the spans as machine segments on `tier` (the utterance tier when
/// None), one txn, skipping any that overlap a live segment there.
pub fn commit(session: &mut Session, text: &str, tier: Option<&str>, spans: &[(f64, f64)]) -> Result<AutoSegmented, String> {
    let p = session::project(session)?;
    let tier = match tier {
        Some(t) => t.to_string(),
        None => session::utterance_tier(p)?,
    };
    let index = session::next_pos_index(p, text)?;
    let (ids, res) = p.add_segments(text, &tier, index, spans)?;
    let states = session::states(p, &ids)?;
    Ok(AutoSegmented { skipped: spans.len() - ids.len(), ids, tier, findings: finding_lines(&res), states })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 16 kHz mono PCM16: 0.5 s quiet, 0.8 s tone, 0.6 s quiet, 1 s tone, 0.5 s quiet.
    fn write_wav(path: &std::path::Path) {
        let mut samples = Vec::new();
        for (ms, amp) in [(500, 0.0), (800, 0.3), (600, 0.0), (1000, 0.3), (500, 0.0)] {
            for i in 0..ms * 16 {
                let v = amp * (2.0 * std::f64::consts::PI * 220.0 * i as f64 / 16000.0).sin() + if i % 2 == 0 { 0.001 } else { -0.001 };
                samples.push((v * 32767.0) as i16);
            }
        }
        let data = (samples.len() * 2) as u32;
        let mut f = fs::File::create(path).unwrap();
        for chunk in [&b"RIFF"[..], &(36 + data).to_le_bytes(), b"WAVEfmt ", &16u32.to_le_bytes(), &1u16.to_le_bytes(), &1u16.to_le_bytes(), &16000u32.to_le_bytes(), &32000u32.to_le_bytes(), &2u16.to_le_bytes(), &16u16.to_le_bytes(), b"data", &data.to_le_bytes()] {
            f.write_all(chunk).unwrap();
        }
        for s in samples {
            f.write_all(&s.to_le_bytes()).unwrap();
        }
    }

    #[test]
    fn spans_are_computed_once_cached_and_written_as_one_txn() {
        let root = std::env::temp_dir().join(format!("wugs-seg-test-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, root.to_str().unwrap(), "prs_Test").unwrap();
        let wav = root.join("in.wav");
        fs::create_dir_all(&root).unwrap();
        write_wav(&wav);
        let source = session::import_media(&mut s, wav.to_str().unwrap()).unwrap().id;
        let text = session::create_text(&mut s, "t").unwrap();

        let job = job(&mut s, &source).unwrap();
        let found = spans(&Provider::VoiceActivity, &job).unwrap();
        assert_eq!(found.len(), 2, "{found:?}");
        let cached: Vec<_> = fs::read_dir(&job.dir).unwrap().collect();
        assert_eq!(cached.len(), 1, "one cache file");
        assert_eq!(spans(&Provider::VoiceActivity, &job).unwrap(), found, "the cache gives the same spans");

        // A segment the person drew over the first span is left alone.
        let drawn = session::add_segment(&mut s, &text, None, Some(450.0), Some(1350.0), "mine").unwrap().id;
        let out = commit(&mut s, &text, None, &found).unwrap();
        assert_eq!((out.ids.len(), out.skipped), (1, 1));
        let detail = session::get_text(&mut s, &text).unwrap();
        assert_eq!(detail.segments.len(), 2);
        assert!(detail.segments.iter().any(|r| r.id == drawn && r.content == "mine"));

        // Undo deletes the untouched ones; one typed into since is kept.
        let checks = |st: &[SegmentStateView]| -> Vec<session::SegmentCheckView> {
            st.iter()
                .map(|x| session::SegmentCheckView { id: x.id.clone(), deleted: Some(x.deleted), start_ms: x.start_ms, end_ms: x.end_ms, content: Some(x.content.clone()), words: Some(x.words) })
                .collect()
        };
        assert_eq!(out.states.len(), 1);
        assert!(session::delete_segments(&mut s, &text, &checks(&out.states), false).is_ok());
        let back = session::restore_segments(&mut s, &text, &out.ids).unwrap();
        session::edit_segment_content(&mut s, &text, &out.ids[0], "typed later").unwrap();
        let err = session::delete_segments(&mut s, &text, &checks(&back.states), false).unwrap_err();
        assert!(err.contains("edited since"), "{err}");
        let partial = session::delete_segments(&mut s, &text, &checks(&back.states), true).unwrap();
        assert_eq!((partial.deleted.len(), partial.kept.len()), (0, 1));
        assert_eq!(session::get_text(&mut s, &text).unwrap().segments.len(), 2, "nothing typed is lost");
        drop(s);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn split_and_merge_through_the_session() {
        let root = std::env::temp_dir().join(format!("wugs-seg-split-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, root.to_str().unwrap(), "prs_Test").unwrap();
        let text = session::create_text(&mut s, "t").unwrap();
        let a = session::add_segment(&mut s, &text, None, Some(0.0), Some(2000.0), "one").unwrap().id;
        let b = session::split_segment(&mut s, &text, &a, 1200.4).unwrap().id;
        let rows = session::get_text(&mut s, &text).unwrap().segments;
        assert_eq!(rows.iter().map(|r| (r.id.clone(), r.start_ms, r.end_ms)).collect::<Vec<_>>(), [(a.clone(), Some(0.0), Some(1200.0)), (b.clone(), Some(1200.0), Some(2000.0))]);
        let merged = session::merge_segments(&mut s, &text, &a, &b, &[]).unwrap();
        let rows = session::get_text(&mut s, &text).unwrap().segments;
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].end_ms, rows[0].content.as_str()), (Some(2000.0), "one"));
        assert!(merged.states[1].deleted);
        // The undo of the merge after the text was changed elsewhere is refused.
        session::edit_segment_content(&mut s, &text, &a, "one more").unwrap();
        let expect = [session::SegmentCheckView { id: a.clone(), end_ms: Some(2000.0), content: Some("one".into()), ..Default::default() }];
        assert!(session::unmerge_segments(&mut s, &text, &a, &b, 1200.0, Some("one"), &expect).unwrap_err().contains("edited since"));
        assert_eq!(session::get_text(&mut s, &text).unwrap().segments.len(), 1, "nothing written");
        let expect = [session::SegmentCheckView { id: a.clone(), end_ms: Some(2000.0), content: Some("one more".into()), ..Default::default() }];
        let apart = session::unmerge_segments(&mut s, &text, &a, &b, 1200.0, None, &expect).unwrap();
        assert_eq!((apart.states[0].end_ms, apart.states[1].deleted), (Some(1200.0), false));
        drop(s);
        fs::remove_dir_all(root).ok();
    }
}
