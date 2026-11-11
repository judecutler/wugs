//! Speech spans by voice activity (roadmap 59): where speech starts and ends
//! in a whole recording, offline, with no model to download. It powers the
//! built-in segmentation provider and the timeline's edge snapping.
//!
//! Energy-based: the recording is read once, streamed in fixed buffers
//! (an hours-long WAV is never held in memory), mixed to mono, and measured
//! in short frames as RMS level in dBFS. A frame is speech when it is a
//! margin above the recording's own noise floor (a low percentile of the
//! frame levels), so a hissy field recording and a clean studio one both
//! work. Speech runs separated by less than a real pause are joined (the
//! hangover: stops and short closures inside a word stay inside it), runs
//! too short to be a syllable are dropped, and each span is padded so a
//! quiet onset such as the fricative of "three" is kept inside it. Pads
//! never make two spans overlap.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use wugs_peaks::WavReader;

use crate::Error;

/// How speech is told from silence. Every field is part of [`Params::key`],
/// so a cache keyed by it never mixes results of two settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Analysis frame, ms.
    pub frame_ms: f64,
    /// A frame is speech when its level is this many dB over the floor.
    pub margin_db: f32,
    /// The noise floor is this quantile (0..1) of the frame levels.
    pub floor_quantile: f32,
    /// Speech runs shorter than this, after joining, are dropped, ms.
    pub min_speech_ms: f64,
    /// Only a silence at least this long splits two spans, ms.
    pub min_silence_ms: f64,
    /// Each span is widened by this much on both sides, ms.
    pub pad_ms: f64,
}

impl Default for Params {
    fn default() -> Params {
        Params { frame_ms: 20.0, margin_db: 12.0, floor_quantile: 0.1, min_speech_ms: 120.0, min_silence_ms: 250.0, pad_ms: 100.0 }
    }
}

impl Params {
    /// A short stable name for these settings, for cache keys.
    pub fn key(&self) -> String {
        format!(
            "e1-f{}-m{}-q{}-s{}-g{}-p{}",
            self.frame_ms, self.margin_db, self.floor_quantile, self.min_speech_ms, self.min_silence_ms, self.pad_ms
        )
    }
}

/// Levels at or below this are digital silence; the floor never goes lower,
/// or a recording with zeroed stretches would call its breaths speech.
const FLOOR_DB: f32 = -80.0;
/// A recording whose loud frames stay under this holds no speech at all.
const SILENT_DB: f32 = -60.0;

/// The speech spans of a whole WAV file, as `(start_ms, end_ms)` in time
/// order, padded and never overlapping.
pub fn speech_spans(path: &Path, params: Params) -> Result<Vec<(f64, f64)>, Error> {
    let (levels, duration_ms) = frame_levels(path, params.frame_ms)?;
    Ok(spans_from_levels(&levels, params, duration_ms))
}

/// Per-frame RMS level in dBFS of a WAV file (all channels mixed), with the
/// recording's length in ms. Streams the file.
pub fn frame_levels(path: &Path, frame_ms: f64) -> Result<(Vec<f32>, f64), Error> {
    if !(frame_ms > 0.0) {
        return Err(Error::Input(format!("bad frame length {frame_ms} ms")));
    }
    let file = BufReader::new(File::open(path).map_err(|e| Error::Audio(format!("open {}: {e}", path.display())))?);
    let mut reader = WavReader::new(file).map_err(|e| Error::Audio(e.to_string()))?;
    let fmt = reader.format();
    let ch = fmt.channels.max(1) as usize;
    let frame_len = ((fmt.sample_rate as f64 * frame_ms / 1000.0).round() as usize).max(1);
    let mut levels = Vec::new();
    let mut buf = vec![0i16; 8192 * ch];
    let (mut sum, mut n, mut total) = (0f64, 0usize, 0u64);
    loop {
        let got = reader.read_frames(&mut buf).map_err(|e| Error::Audio(e.to_string()))?;
        if got == 0 {
            break;
        }
        total += got as u64;
        for frame in buf[..got * ch].chunks_exact(ch) {
            let mono = frame.iter().map(|&s| s as f64).sum::<f64>() / (ch as f64 * 32768.0);
            sum += mono * mono;
            n += 1;
            if n == frame_len {
                levels.push(db(sum / n as f64));
                (sum, n) = (0.0, 0);
            }
        }
    }
    if n > 0 {
        levels.push(db(sum / n as f64));
    }
    Ok((levels, total as f64 * 1000.0 / fmt.sample_rate as f64))
}

fn db(mean_square: f64) -> f32 {
    (10.0 * (mean_square + 1e-12).log10()) as f32
}

fn quantile(sorted: &[f32], q: f32) -> f32 {
    let i = ((sorted.len() - 1) as f32 * q.clamp(0.0, 1.0)).round() as usize;
    sorted[i]
}

/// The spans for frame levels already measured: the decision part of
/// [`speech_spans`], kept apart so it can be tested on made-up levels.
pub fn spans_from_levels(levels: &[f32], params: Params, duration_ms: f64) -> Vec<(f64, f64)> {
    if levels.is_empty() {
        return Vec::new();
    }
    let mut sorted = levels.to_vec();
    sorted.sort_by(f32::total_cmp);
    let floor = quantile(&sorted, params.floor_quantile).max(FLOOR_DB);
    let loud = quantile(&sorted, 0.95);
    if loud < SILENT_DB {
        return Vec::new();
    }
    let threshold = floor + params.margin_db;
    let fm = params.frame_ms;

    // Speech runs, in frames.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut open: Option<usize> = None;
    for (i, &l) in levels.iter().enumerate() {
        match (l > threshold, open) {
            (true, None) => open = Some(i),
            (false, Some(s)) => {
                runs.push((s, i));
                open = None;
            }
            _ => {}
        }
    }
    if let Some(s) = open {
        runs.push((s, levels.len()));
    }
    // A loud recording with no quiet frames at all (the floor is speech
    // itself) is one span; nothing in it says where to cut.
    if runs.is_empty() && loud - floor < params.margin_db && floor > SILENT_DB {
        runs.push((0, levels.len()));
    }

    // Hangover: a gap shorter than a real pause stays inside the span.
    let mut joined: Vec<(f64, f64)> = Vec::new();
    for (s, e) in runs {
        let (s, e) = (s as f64 * fm, (e as f64 * fm).min(duration_ms));
        match joined.last_mut() {
            Some(last) if s - last.1 < params.min_silence_ms => last.1 = e,
            _ => joined.push((s, e)),
        }
    }
    joined.retain(|(s, e)| e - s >= params.min_speech_ms);

    // Pad, clamped to the recording, and never into a neighbour: two pads
    // that would meet share the gap at its middle.
    let mut out = Vec::with_capacity(joined.len());
    for i in 0..joined.len() {
        let (s, e) = joined[i];
        let lo = match i.checked_sub(1).map(|j| joined[j]) {
            Some((_, prev_end)) => (s - params.pad_ms).max((prev_end + s) / 2.0),
            None => (s - params.pad_ms).max(0.0),
        };
        let hi = match joined.get(i + 1) {
            Some(&(next_start, _)) => (e + params.pad_ms).min((e + next_start) / 2.0),
            None => (e + params.pad_ms).min(duration_ms),
        };
        out.push((lo, hi));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const RATE: u32 = 16_000;

    /// A 16-bit mono (or stereo, same signal) WAV of `parts`: each is a
    /// length in ms and an amplitude (0 for silence), filled with a 220 Hz
    /// tone plus a little noise, so it looks like voiced speech in a room.
    fn wav(name: &str, channels: u16, parts: &[(f64, f64)], hiss: f64) -> std::path::PathBuf {
        let mut samples: Vec<i16> = Vec::new();
        let mut seed = 0x2545_f491_u32;
        let mut t = 0usize;
        for &(ms, amp) in parts {
            for _ in 0..(ms / 1000.0 * RATE as f64) as usize {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let noise = (seed as f64 / u32::MAX as f64 - 0.5) * 2.0 * hiss;
                let tone = amp * (2.0 * std::f64::consts::PI * 220.0 * t as f64 / RATE as f64).sin();
                let v = ((tone + noise) * 32767.0).clamp(-32768.0, 32767.0) as i16;
                for _ in 0..channels {
                    samples.push(v);
                }
                t += 1;
            }
        }
        let dir = std::env::temp_dir().join(format!("wugs-vad-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.wav");
        let data = (samples.len() * 2) as u32;
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"RIFF").unwrap();
        f.write_all(&(36 + data).to_le_bytes()).unwrap();
        f.write_all(b"WAVEfmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap();
        f.write_all(&channels.to_le_bytes()).unwrap();
        f.write_all(&RATE.to_le_bytes()).unwrap();
        f.write_all(&(RATE * 2 * channels as u32).to_le_bytes()).unwrap();
        f.write_all(&(2 * channels).to_le_bytes()).unwrap();
        f.write_all(&16u16.to_le_bytes()).unwrap();
        f.write_all(b"data").unwrap();
        f.write_all(&data.to_le_bytes()).unwrap();
        for s in samples {
            f.write_all(&s.to_le_bytes()).unwrap();
        }
        path
    }

    fn near(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn bursts_between_pauses_become_padded_spans() {
        // 0.5 s room tone, 0.8 s speech, 0.6 s pause, 1.0 s speech, 0.5 s tail.
        let path = wav("bursts", 1, &[(500.0, 0.0), (800.0, 0.3), (600.0, 0.0), (1000.0, 0.2), (500.0, 0.0)], 0.002);
        let spans = speech_spans(&path, Params::default()).unwrap();
        assert_eq!(spans.len(), 2, "{spans:?}");
        // Padded 100 ms outward from 500..1300 and 1900..2900, give or take a frame.
        assert!(near(spans[0].0, 400.0, 20.0) && near(spans[0].1, 1400.0, 20.0), "{spans:?}");
        assert!(near(spans[1].0, 1800.0, 20.0) && near(spans[1].1, 3000.0, 20.0), "{spans:?}");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn a_quiet_onset_is_kept_inside_the_span() {
        // A soft 80 ms "fricative" (noise only, well above the room) right
        // before the loud vowel must not be cut off.
        let path = wav("onset", 1, &[(400.0, 0.0), (80.0, 0.02), (600.0, 0.4), (400.0, 0.0)], 0.001);
        let spans = speech_spans(&path, Params::default()).unwrap();
        assert_eq!(spans.len(), 1, "{spans:?}");
        assert!(spans[0].0 <= 400.0 - 50.0, "the span starts before the onset: {spans:?}");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn short_pauses_join_and_clicks_are_dropped() {
        // Two words 150 ms apart are one span; a 40 ms click is no speech.
        let path = wav(
            "join",
            2,
            &[(300.0, 0.0), (400.0, 0.3), (150.0, 0.0), (400.0, 0.3), (700.0, 0.0), (40.0, 0.5), (700.0, 0.0)],
            0.002,
        );
        let spans = speech_spans(&path, Params::default()).unwrap();
        assert_eq!(spans.len(), 1, "{spans:?}");
        assert!(near(spans[0].0, 200.0, 20.0) && near(spans[0].1, 1350.0, 20.0), "{spans:?}");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn silence_has_no_spans_and_pads_stay_in_the_recording() {
        let path = wav("silent", 1, &[(2000.0, 0.0)], 0.0);
        assert!(speech_spans(&path, Params::default()).unwrap().is_empty());
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
        let path = wav("edges", 1, &[(600.0, 0.3), (500.0, 0.0), (600.0, 0.3)], 0.002);
        let spans = speech_spans(&path, Params::default()).unwrap();
        assert_eq!(spans.len(), 2, "{spans:?}");
        assert_eq!(spans[0].0, 0.0);
        assert!(near(spans[1].1, 1700.0, 1.0), "{spans:?}");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn pads_never_overlap_neighbours() {
        let p = Params { pad_ms: 400.0, ..Params::default() };
        // Frames of 20 ms: speech 10..30 and 45..60 (a 300 ms gap).
        let mut levels = vec![-70.0f32; 80];
        for i in (10..30).chain(45..60) {
            levels[i] = -20.0;
        }
        let spans = spans_from_levels(&levels, p, 1600.0);
        assert_eq!(spans.len(), 2, "{spans:?}");
        assert_eq!(spans[0].1, spans[1].0, "the pads meet in the middle of the gap: {spans:?}");
        assert_eq!(spans[0].1, 750.0);
        assert_eq!(spans[0].0, 0.0);
        assert_eq!(spans[1].1, 1600.0);
    }

    #[test]
    fn the_key_names_every_setting() {
        assert_ne!(Params::default().key(), Params { pad_ms: 90.0, ..Params::default() }.key());
        assert_ne!(Params::default().key(), Params { margin_db: 10.0, ..Params::default() }.key());
    }
}
