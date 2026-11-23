//! Pitch and intensity built in: the `acoustic-analysis` capability's
//! `voice-analysis` provider, for a computer without Praat. It follows
//! Praat's own methods and defaults so the curves read the same:
//!
//! - **Pitch**: Boersma's (1993) autocorrelation method, To Pitch (ac) with
//!   75–600 Hz, a window of three periods of the floor, the windowed signal's
//!   autocorrelation divided by the window's, candidates at its peaks
//!   (parabolic interpolation), voicing threshold 0.45, silence threshold
//!   0.03, octave cost 0.01, and the best path through the candidates by
//!   Viterbi with octave-jump cost 0.35 and voiced/unvoiced cost 0.14.
//! - **Intensity**: mean square in a window of 3.2 periods of 75 Hz, the
//!   mean taken off, in dB re 2·10⁻⁵ Pa (samples as Pa), as Praat's To
//!   Intensity does.
//!
//! Formants are not computed here: with this provider they stay empty and
//! the toolbar says Praat draws them. Chunks match `praat_analysis` (10 s,
//! a frame every 10 ms) and are not cached on disk: a chunk takes tens of
//! milliseconds.

use std::fs::File;
use std::path::Path;

use realfft::RealFftPlanner;

use crate::praat_analysis::{Chunk, CHUNK_MS, MAX_CHUNKS, STEP_MS};

const MIN_PITCH: f64 = 75.0;
const MAX_PITCH: f64 = 600.0;
const VOICING_THRESHOLD: f64 = 0.45;
const SILENCE_THRESHOLD: f64 = 0.03;
const OCTAVE_COST: f64 = 0.01;
const OCTAVE_JUMP_COST: f64 = 0.35;
const VOICED_UNVOICED_COST: f64 = 0.14;
const MAX_CANDIDATES: usize = 15;

/// The recording's samples from `start_ms` to `end_ms` mixed to mono
/// (zeros past either end, as the spectrogram reads them), with its rate and
/// its length in ms.
fn read(wav: &Path, start_ms: f64, end_ms: f64) -> Result<(Vec<f64>, f64, f64), String> {
    let frames = {
        let mut f = File::open(wav).map_err(|e| e.to_string())?;
        wugs_peaks::WavReader::new(&mut f).map_err(|e| e.to_string())?.declared_frames()
    };
    let (samples, rate) = crate::spectrogram::read_mono(wav, start_ms, end_ms)?;
    let rate = f64::from(rate);
    Ok((samples.into_iter().map(f64::from).collect(), rate, frames.map_or(f64::INFINITY, |n| n as f64 / rate * 1000.0)))
}

fn hann(n: usize) -> Vec<f64> {
    (0..n).map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * (i as f64 + 0.5) / n as f64).cos()).collect()
}

#[derive(Clone, Copy)]
struct Candidate {
    /// Hz; 0 for the unvoiced candidate.
    hz: f64,
    strength: f64,
}

/// Every frame's candidates, the unvoiced one first.
fn candidates(samples: &[f64], rate: f64, centres: &[usize]) -> Vec<Vec<Candidate>> {
    let win = (3.0 / MIN_PITCH * rate).round() as usize;
    let window = hann(win);
    let size = (2 * win).next_power_of_two();
    let mut planner = RealFftPlanner::<f64>::new();
    let forward = planner.plan_fft_forward(size);
    let inverse = planner.plan_fft_inverse(size);
    let mut input = forward.make_input_vec();
    let mut spectrum = forward.make_output_vec();
    let mut back = inverse.make_output_vec();
    // The window's own autocorrelation, to divide by (Boersma's r_w).
    let mut auto = |x: &[f64], out: &mut Vec<f64>| {
        input.iter_mut().for_each(|v| *v = 0.0);
        input[..x.len()].copy_from_slice(x);
        forward.process(&mut input, &mut spectrum).ok();
        for c in spectrum.iter_mut() {
            *c = realfft::num_complex::Complex::new(c.norm_sqr(), 0.0);
        }
        inverse.process(&mut spectrum, &mut back).ok();
        out.clear();
        out.extend(back.iter().take(win).map(|v| v / back[0].max(1e-300)));
    };
    let mut rw = Vec::new();
    auto(&window, &mut rw);
    let global_peak = samples.iter().fold(0f64, |m, v| m.max(v.abs())).max(1e-12);
    let min_lag = (rate / MAX_PITCH).floor().max(2.0) as usize;
    let max_lag = ((rate / MIN_PITCH).ceil() as usize).min(win / 2);
    let mut ra = Vec::new();
    let mut frame = vec![0f64; win];
    centres
        .iter()
        .map(|&centre| {
            let start = centre as i64 - (win / 2) as i64;
            for (i, v) in frame.iter_mut().enumerate() {
                let at = start + i as i64;
                *v = if at >= 0 && (at as usize) < samples.len() { samples[at as usize] } else { 0.0 };
            }
            let mean = frame.iter().sum::<f64>() / win as f64;
            let local_peak = frame.iter().fold(0f64, |m, v| m.max((v - mean).abs()));
            let unvoiced = VOICING_THRESHOLD + (2.0 - (local_peak / global_peak) / (SILENCE_THRESHOLD / (1.0 + VOICING_THRESHOLD))).max(0.0);
            let mut out = vec![Candidate { hz: 0.0, strength: unvoiced }];
            if local_peak == 0.0 {
                return out;
            }
            let windowed: Vec<f64> = frame.iter().zip(&window).map(|(v, w)| (v - mean) * w).collect();
            auto(&windowed, &mut ra);
            let r = |lag: usize| ra[lag] / rw[lag].max(1e-12);
            for lag in min_lag.max(1)..max_lag {
                let (a, b, c) = (r(lag - 1), r(lag), r(lag + 1));
                if b > a && b >= c && b > 0.5 * VOICING_THRESHOLD {
                    // Parabolic interpolation of the peak.
                    let d = 0.5 * (a - c) / (a - 2.0 * b + c).min(-1e-12);
                    let peak_lag = lag as f64 + d;
                    let peak = (b - 0.25 * (a - c) * d).min(1.0);
                    let hz = rate / peak_lag;
                    if (MIN_PITCH..=MAX_PITCH).contains(&hz) {
                        out.push(Candidate { hz, strength: peak - OCTAVE_COST * (MIN_PITCH / hz).log2() });
                    }
                }
            }
            if out.len() > MAX_CANDIDATES {
                out[1..].sort_by(|x, y| y.strength.total_cmp(&x.strength));
                out.truncate(MAX_CANDIDATES);
            }
            out
        })
        .collect()
}

/// The best path through the candidates (Praat's path finder): each frame's
/// Hz, or None when unvoiced.
fn best_path(frames: &[Vec<Candidate>]) -> Vec<Option<f64>> {
    if frames.is_empty() {
        return Vec::new();
    }
    let cost = |a: Candidate, b: Candidate| -> f64 {
        match (a.hz > 0.0, b.hz > 0.0) {
            (false, false) => 0.0,
            (true, true) => OCTAVE_JUMP_COST * (a.hz / b.hz).log2().abs(),
            _ => VOICED_UNVOICED_COST,
        }
    };
    let mut score: Vec<f64> = frames[0].iter().map(|c| c.strength).collect();
    let mut from: Vec<Vec<usize>> = vec![vec![0; frames[0].len()]];
    for w in frames.windows(2) {
        let (prev, cur) = (&w[0], &w[1]);
        let mut next = Vec::with_capacity(cur.len());
        let mut back = Vec::with_capacity(cur.len());
        for &c in cur {
            let (best, s) = prev.iter().enumerate().map(|(i, &p)| (i, score[i] - cost(p, c))).max_by(|x, y| x.1.total_cmp(&y.1)).unwrap();
            next.push(s + c.strength);
            back.push(best);
        }
        score = next;
        from.push(back);
    }
    let mut at = score.iter().enumerate().max_by(|x, y| x.1.total_cmp(y.1)).map(|(i, _)| i).unwrap_or(0);
    let mut out = vec![None; frames.len()];
    for f in (0..frames.len()).rev() {
        let c = frames[f][at];
        out[f] = (c.hz > 0.0).then_some(c.hz);
        at = from[f][at];
    }
    out
}

/// Each frame's intensity in dB, None in digital silence.
fn intensity(samples: &[f64], rate: f64, centres: &[usize]) -> Vec<Option<f64>> {
    let win = (3.2 / MIN_PITCH * rate).round() as usize;
    let window = hann(win);
    let sum_w: f64 = window.iter().sum();
    centres
        .iter()
        .map(|&centre| {
            let start = centre as i64 - (win / 2) as i64;
            let at = |i: usize| {
                let k = start + i as i64;
                if k >= 0 && (k as usize) < samples.len() { samples[k as usize] } else { 0.0 }
            };
            let mean = (0..win).map(|i| at(i) * window[i]).sum::<f64>() / sum_w;
            let power = (0..win).map(|i| (at(i) - mean).powi(2) * window[i]).sum::<f64>() / sum_w;
            (power > 0.0).then(|| 10.0 * (power / 4e-10).log10())
        })
        .collect()
}

/// One chunk's pitch and intensity.
pub fn chunk(wav: &Path, index: u32) -> Result<Chunk, String> {
    let start_ms = f64::from(index) * f64::from(CHUNK_MS);
    let pad_ms = 3.2 / MIN_PITCH * 1000.0;
    let (samples, rate, total_ms) = read(wav, start_ms - pad_ms, start_ms + f64::from(CHUNK_MS) + pad_ms)?;
    let step = f64::from(STEP_MS);
    let frames = (((total_ms - start_ms) / step).floor().max(0.0) as usize).min((CHUNK_MS / STEP_MS) as usize);
    let centres: Vec<usize> = (0..frames).map(|i| ((pad_ms + (i as f64 + 0.5) * step) / 1000.0 * rate).round() as usize).collect();
    let pitch = best_path(&candidates(&samples, rate, &centres));
    let db = intensity(&samples, rate, &centres);
    Ok(Chunk {
        index,
        start_ms,
        step_ms: step,
        pitch: pitch.into_iter().map(|v| v.map(|x| x as f32)).collect(),
        intensity: db.into_iter().map(|v| v.map(|x| x as f32)).collect(),
        f1: vec![None; frames],
        f2: vec![None; frames],
        f3: vec![None; frames],
    })
}

pub fn analyse(wav: &Path, chunks: &[u32]) -> Result<Vec<Chunk>, String> {
    if chunks.len() > MAX_CHUNKS {
        return Err(format!("at most {MAX_CHUNKS} chunks at a time"));
    }
    chunks.iter().map(|&n| chunk(wav, n)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mono 16-bit WAV: a voice-like buzz (harmonics with falling
    /// amplitude) at `f0[i]` for each second `i`, 0 Hz meaning silence.
    fn voice(path: &Path, rate: u32, f0: &[f64], amplitude: f64) {
        let mut samples: Vec<i16> = Vec::new();
        let mut phase = 0.0f64;
        for &f in f0 {
            for _ in 0..rate {
                let mut v = 0.0;
                if f > 0.0 {
                    phase += f / f64::from(rate);
                    for h in 1..=8 {
                        v += (std::f64::consts::TAU * phase * f64::from(h)).sin() / f64::from(h);
                    }
                }
                samples.push((v * amplitude * 32767.0 / 2.0) as i16);
            }
        }
        let bytes = (samples.len() * 2) as u32;
        let mut w = Vec::new();
        for (tag, val) in [(&b"RIFF"[..], 36 + bytes)] {
            w.extend_from_slice(tag);
            w.extend_from_slice(&val.to_le_bytes());
        }
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&rate.to_le_bytes());
        w.extend_from_slice(&(rate * 2).to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&bytes.to_le_bytes());
        for s in samples {
            w.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(path, w).unwrap();
    }

    #[test]
    fn pitch_follows_the_voice_and_silence_is_unvoiced() {
        let dir = std::env::temp_dir().join(format!("wugs-voice-{}", wugs_model::ulid()));
        std::fs::create_dir_all(&dir).unwrap();
        for rate in [16_000u32, 44_100] {
            let wav = dir.join(format!("v-{rate}.wav"));
            voice(&wav, rate, &[120.0, 220.0, 0.0, 400.0], 0.5);
            let c = chunk(&wav, 0).unwrap();
            assert_eq!(c.pitch.len(), 400, "four seconds of 10 ms frames, no more");
            let at = |s: f64| c.pitch[(s * 100.0) as usize];
            for (s, want) in [(0.5, 120.0), (1.5, 220.0), (3.5, 400.0)] {
                let got = f64::from(at(s).unwrap_or_else(|| panic!("{rate}: unvoiced at {s} s")));
                assert!((got - want).abs() < want * 0.02, "{rate} Hz rate, {s} s: {got} against {want}");
            }
            assert_eq!(at(2.5), None, "silence");
            // Intensity: loud while voiced, nothing in digital silence.
            let db = |s: f64| c.intensity[(s * 100.0) as usize];
            assert!(db(0.5).unwrap() > 70.0, "{:?}", db(0.5));
            assert_eq!(db(2.5), None);
            assert!(c.f1.iter().all(Option::is_none), "formants are Praat's");
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn intensity_is_in_praats_decibels() {
        let dir = std::env::temp_dir().join(format!("wugs-voice-db-{}", wugs_model::ulid()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("sine.wav");
        // A pure 1 kHz sine of amplitude 0.5: mean square 0.125, 10·log10(0.125 / 4e-10) = 84.9 dB.
        let rate = 16_000u32;
        let samples: Vec<i16> = (0..rate).map(|i| ((f64::from(i) / f64::from(rate) * 1000.0 * std::f64::consts::TAU).sin() * 0.5 * 32768.0) as i16).collect();
        let bytes = (samples.len() * 2) as u32;
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + bytes).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        for v in [16u32] {
            w.extend_from_slice(&v.to_le_bytes());
        }
        for v in [1u16, 1u16] {
            w.extend_from_slice(&v.to_le_bytes());
        }
        w.extend_from_slice(&rate.to_le_bytes());
        w.extend_from_slice(&(rate * 2).to_le_bytes());
        for v in [2u16, 16u16] {
            w.extend_from_slice(&v.to_le_bytes());
        }
        w.extend_from_slice(b"data");
        w.extend_from_slice(&bytes.to_le_bytes());
        for s in samples {
            w.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(&wav, w).unwrap();
        let c = chunk(&wav, 0).unwrap();
        let mid = f64::from(c.intensity[50].unwrap());
        assert!((mid - 84.9).abs() < 0.5, "{mid}");
        std::fs::remove_dir_all(dir).ok();
    }

    /// Against Praat itself on the tutorial recording (a real voice), when
    /// Praat's Python build is installed (`pip install praat-parselmouth`);
    /// skipped otherwise. Measured 2026-11-23: pitch within 0.06% at the
    /// median and 0.8% at the 95th percentile, voicing different on 2% of
    /// frames (all at voicing edges), intensity within 0.06 dB at the median.
    #[test]
    fn matches_praat_on_a_real_voice() {
        let wav = Path::new(env!("CARGO_MANIFEST_DIR")).join("tutorial/counting-to-six.wav");
        let script = "import sys, parselmouth\ns = parselmouth.Sound(sys.argv[1])\np = s.to_pitch_ac(time_step=0.01, pitch_floor=75, pitch_ceiling=600)\ni = s.to_intensity(minimum_pitch=75, time_step=0.01)\nfor k in range(int(s.duration * 100)):\n    t = (k + 0.5) / 100\n    print(p.get_value_at_time(t), i.get_value(t))\n";
        let Ok(out) = std::process::Command::new("python3").arg("-c").arg(script).arg(&wav).output() else { return };
        if !out.status.success() {
            eprintln!("parselmouth is not installed; skipped");
            return;
        }
        let praat: Vec<(Option<f64>, Option<f64>)> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| {
                let mut v = l.split(' ').map(|x| x.parse::<f64>().ok().filter(|x| x.is_finite()));
                (v.next().flatten(), v.next().flatten())
            })
            .collect();
        let ours = chunk(&wav, 0).unwrap();
        let (mut pitch, mut db, mut voicing, mut frames) = (Vec::new(), Vec::new(), 0, 0);
        // Leave out the first and last 0.1 s, where the windows run off the recording.
        for k in 10..praat.len().saturating_sub(10).min(ours.pitch.len()) {
            frames += 1;
            let (pp, pi) = praat[k];
            match (ours.pitch[k], pp) {
                (Some(a), Some(b)) => pitch.push((f64::from(a) - b).abs() / b),
                (None, None) => {}
                _ => voicing += 1,
            }
            if let (Some(a), Some(b)) = (ours.intensity[k], pi) {
                db.push((f64::from(a) - b).abs());
            }
        }
        let pct = |v: &mut Vec<f64>, q: f64| {
            v.sort_by(f64::total_cmp);
            v[((v.len() - 1) as f64 * q) as usize]
        };
        assert!(frames > 400 && pitch.len() > 100, "{frames} frames, {} voiced", pitch.len());
        assert!(pct(&mut pitch, 0.5) < 0.002 && pct(&mut pitch, 0.95) < 0.02, "pitch {:?}", (pct(&mut pitch, 0.5), pct(&mut pitch, 0.95)));
        assert!(voicing * 100 < frames * 4, "{voicing} of {frames} frames voiced differently");
        assert!(pct(&mut db, 0.5) < 0.2 && pct(&mut db, 0.95) < 1.0, "intensity {:?}", (pct(&mut db, 0.5), pct(&mut db, 0.95)));
    }
}

