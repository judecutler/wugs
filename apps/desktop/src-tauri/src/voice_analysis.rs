//! Pitch, intensity and formants built in: the `acoustic-analysis` capability's
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
//! - **Formants**: To Formant (burg) with 5 formants up to 5500 Hz: the
//!   sound resampled to 11 kHz (here by FFT), pre-emphasis from 50 Hz, a
//!   Gaussian window of 50 ms (25 ms effective), Burg's LPC of order 10,
//!   the polynomial's roots moved into the unit circle, and the first three
//!   frequencies between 50 Hz and 50 Hz below the Nyquist.
//!
//! Chunks match `praat_analysis` (10 s, a frame every 10 ms) and are not
//! cached on disk: a chunk takes about 0.1 s (release build, 44.1 kHz).

use std::fs::File;
use std::path::Path;

use realfft::num_complex::Complex;
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

const FORMANT_CEILING: f64 = 5500.0;
const POLES: usize = 10;
const FORMANT_WINDOW: f64 = 0.025;
const PRE_EMPHASIS_FROM: f64 = 50.0;
const SAFETY_HZ: f64 = 50.0;

/// `samples` at `rate` resampled to `to` by the FFT: the band above the
/// lower Nyquist dropped, as a band-limited resampling does.
fn resample(samples: &[f64], rate: f64, to: f64) -> Vec<f64> {
    let n = samples.len();
    let m = ((n as f64) * to / rate).round() as usize;
    if n < 2 || m < 2 {
        return vec![0.0; m];
    }
    let mut planner = RealFftPlanner::<f64>::new();
    let forward = planner.plan_fft_forward(n);
    let inverse = planner.plan_fft_inverse(m);
    let mut input = samples.to_vec();
    let mut spectrum = forward.make_output_vec();
    forward.process(&mut input, &mut spectrum).expect("sizes from the plan");
    let mut out_spec = inverse.make_input_vec();
    let keep = out_spec.len().min(spectrum.len());
    out_spec[..keep].copy_from_slice(&spectrum[..keep]);
    // The inverse wants real values at 0 Hz and, for an even length, at the Nyquist.
    out_spec[0].im = 0.0;
    if m % 2 == 0 {
        let last = out_spec.len() - 1;
        out_spec[last].im = 0.0;
    }
    let mut out = inverse.make_output_vec();
    inverse.process(&mut out_spec, &mut out).expect("sizes from the plan");
    let scale = 1.0 / n as f64;
    out.iter_mut().for_each(|v| *v *= scale);
    out
}

/// Burg's linear prediction coefficients of order `m` (Praat's `VECburg`),
/// or None for a frame with no signal.
fn burg(x: &[f64], m: usize) -> Option<Vec<f64>> {
    let n = x.len();
    if n <= m + 1 {
        return None;
    }
    let mut a = vec![0.0; m + 1];
    let mut aa = vec![0.0; m + 1];
    let p: f64 = x.iter().map(|v| v * v).sum();
    if p <= 0.0 {
        return None;
    }
    // One-based like the description, so the indices read the same.
    let mut b1 = vec![0.0; n + 1];
    let mut b2 = vec![0.0; n + 1];
    b1[1] = x[0];
    b2[n - 1] = x[n - 1];
    for j in 2..n {
        b1[j] = x[j - 1];
        b2[j - 1] = x[j - 1];
    }
    for i in 1..=m {
        let (mut num, mut den) = (0.0, 0.0);
        for j in 1..=n - i {
            num += b1[j] * b2[j];
            den += b1[j] * b1[j] + b2[j] * b2[j];
        }
        if den <= 0.0 {
            return None;
        }
        a[i] = 2.0 * num / den;
        for j in 1..i {
            a[j] = aa[j] - a[i] * aa[i - j];
        }
        if i < m {
            aa[1..=i].copy_from_slice(&a[1..=i]);
            for j in 1..n - i {
                b1[j] -= aa[i] * b2[j];
                b2[j] = b2[j + 1] - aa[i] * b1[j + 1];
            }
        }
    }
    Some(a)
}

/// The roots of the polynomial with `coef` (lowest power first, the last
/// one 1), by Aberth–Ehrlich iteration.
fn roots(coef: &[f64]) -> Vec<Complex<f64>> {
    let deg = coef.len() - 1;
    let eval = |z: Complex<f64>| {
        let (mut p, mut d) = (Complex::new(0.0, 0.0), Complex::new(0.0, 0.0));
        for &c in coef.iter().rev() {
            d = d * z + p;
            p = p * z + c;
        }
        (p, d)
    };
    // Start on a circle of the roots' typical size, off the real axis.
    let radius = coef[..deg].iter().map(|c| c.abs()).fold(0.0f64, f64::max).powf(1.0 / deg as f64).max(0.5);
    let mut z: Vec<Complex<f64>> = (0..deg).map(|k| Complex::from_polar(radius, std::f64::consts::TAU * (k as f64 + 0.25) / deg as f64)).collect();
    for _ in 0..500 {
        let mut moved = 0.0f64;
        for k in 0..deg {
            let (p, d) = eval(z[k]);
            if p.norm() == 0.0 {
                continue;
            }
            let ratio = p / d;
            let repulsion: Complex<f64> = (0..deg).filter(|&j| j != k).map(|j| Complex::new(1.0, 0.0) / (z[k] - z[j])).sum();
            let step = ratio / (Complex::new(1.0, 0.0) - ratio * repulsion);
            if step.is_finite() {
                z[k] -= step;
                moved = moved.max(step.norm());
            }
        }
        if moved < 1e-14 {
            break;
        }
    }
    z
}

/// Each frame's first three formants (Hz), as Praat's To Formant (burg)
/// finds them; `times` are seconds into `samples`.
fn formants(samples: &[f64], rate: f64, times: &[f64]) -> Vec<[Option<f64>; 3]> {
    let to = 2.0 * FORMANT_CEILING;
    let mut x = resample(samples, rate, to);
    let factor = (-std::f64::consts::TAU * PRE_EMPHASIS_FROM / to).exp();
    for i in (1..x.len()).rev() {
        x[i] -= factor * x[i - 1];
    }
    let half = ((2.0 * FORMANT_WINDOW * to).floor() as usize) / 2;
    let n = 2 * half;
    let edge = (-12.0f64).exp();
    let window: Vec<f64> = (1..=n)
        .map(|i| {
            let mid = 0.5 * (n as f64 + 1.0);
            let d = i as f64 - mid;
            ((-48.0 * d * d / ((n as f64 + 1.0) * (n as f64 + 1.0))).exp() - edge) / (1.0 - edge)
        })
        .collect();
    let nyquist = to / 2.0;
    let mut frame = vec![0.0; n];
    times
        .iter()
        .map(|&t| {
            // Praat puts sample i at (i + 0.5) / rate, so its "low index" of t is this.
            let left = (t * to - 0.5).floor() as i64;
            let start = left + 1 - half as i64;
            for (j, v) in frame.iter_mut().enumerate() {
                let k = start + j as i64;
                *v = if k >= 0 && (k as usize) < x.len() { x[k as usize] * window[j] } else { 0.0 };
            }
            let mut out = [None; 3];
            let Some(a) = burg(&frame, POLES) else { return out };
            let mut coef: Vec<f64> = (1..=POLES).map(|i| -a[POLES - i + 1]).collect();
            coef.push(1.0);
            let mut found: Vec<f64> = roots(&coef)
                .into_iter()
                .filter(|r| r.im >= 0.0)
                .map(|r| {
                    // Into the unit circle, as Praat's Roots_fixIntoUnitCircle.
                    let r = if r.norm() > 1.0 { Complex::new(1.0, 0.0) / r.conj() } else { r };
                    r.im.atan2(r.re).abs() * nyquist / std::f64::consts::PI
                })
                .filter(|f| *f >= SAFETY_HZ && *f <= nyquist - SAFETY_HZ)
                .collect();
            found.sort_by(f64::total_cmp);
            for (slot, f) in out.iter_mut().zip(found) {
                *slot = Some(f);
            }
            out
        })
        .collect()
}

/// One chunk's pitch, intensity and formants.
pub fn chunk(wav: &Path, index: u32) -> Result<Chunk, String> {
    let start_ms = f64::from(index) * f64::from(CHUNK_MS);
    let pad_ms = 3.2 / MIN_PITCH * 1000.0;
    let (samples, rate, total_ms) = read(wav, start_ms - pad_ms, start_ms + f64::from(CHUNK_MS) + pad_ms)?;
    let step = f64::from(STEP_MS);
    let frames = (((total_ms - start_ms) / step).floor().max(0.0) as usize).min((CHUNK_MS / STEP_MS) as usize);
    let centres: Vec<usize> = (0..frames).map(|i| ((pad_ms + (i as f64 + 0.5) * step) / 1000.0 * rate).round() as usize).collect();
    let pitch = best_path(&candidates(&samples, rate, &centres));
    let db = intensity(&samples, rate, &centres);
    let times: Vec<f64> = (0..frames).map(|i| (pad_ms + (i as f64 + 0.5) * step) / 1000.0).collect();
    let formant = formants(&samples, rate, &times);
    let f = |k: usize| formant.iter().map(|v| v[k].map(|x| x as f32)).collect();
    Ok(Chunk {
        index,
        start_ms,
        step_ms: step,
        pitch: pitch.into_iter().map(|v| v.map(|x| x as f32)).collect(),
        intensity: db.into_iter().map(|v| v.map(|x| x as f32)).collect(),
        f1: f(0),
        f2: f(1),
        f3: f(2),
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
    /// A vowel made by a source–filter synthesizer (a 120 Hz pulse train,
    /// a glottal roll-off of two 100 Hz low-passes, resonators at 700, 1220,
    /// 2600, 3300 and 4200 Hz) comes back with its first three formants, at
    /// 16 and 44.1 kHz. (Praat gives 707/1214/2634 and 710/1218/2613 for the
    /// same signal; without the roll-off or the upper resonators both find
    /// a spare pole between F2 and F3.)
    #[test]
    fn formants_of_a_synthetic_vowel() {
        for rate in [16_000.0f64, 44_100.0] {
            let n = (rate * 0.5) as usize;
            let mut x: Vec<f64> = (0..n).map(|i| if i % (rate / 120.0).round() as usize == 0 { 1.0 } else { 0.0 }).collect();
            let pole = (-std::f64::consts::TAU * 100.0 / rate).exp();
            for _ in 0..2 {
                let mut y = 0.0;
                for v in x.iter_mut() {
                    y = *v + pole * y;
                    *v = y;
                }
            }
            for (f, bw) in [(700.0, 80.0), (1220.0, 90.0), (2600.0, 120.0), (3300.0, 200.0), (4200.0, 250.0)] {
                // A two-pole resonator (Klatt's).
                let c = -(-2.0 * std::f64::consts::PI * bw / rate).exp();
                let b = 2.0 * (-std::f64::consts::PI * bw / rate).exp() * (std::f64::consts::TAU * f / rate).cos();
                let a = 1.0 - b - c;
                let (mut y1, mut y2) = (0.0, 0.0);
                for v in x.iter_mut() {
                    let y = a * *v + b * y1 + c * y2;
                    y2 = y1;
                    y1 = y;
                    *v = y;
                }
            }
            let peak = x.iter().fold(0f64, |m, v| m.max(v.abs()));
            x.iter_mut().for_each(|v| *v *= 0.5 / peak);
            let got = formants(&x, rate, &[0.25]);
            for (k, want) in [700.0, 1220.0, 2600.0].into_iter().enumerate() {
                let f = got[0][k].unwrap_or_else(|| panic!("{rate}: no F{}", k + 1));
                assert!((f - want).abs() < want * 0.03, "{rate}: F{} {f} against {want}", k + 1);
            }
        }
    }

    /// Formants against Praat's To Formant (burg) at Praat's own frame
    /// times, on the tutorial recording's frames within 30 dB of its
    /// loudest (when parselmouth is installed). Measured 2026-11-23: within
    /// 0.1% at the median and about 1.2% at the 90th percentile for F1–F3;
    /// the rest are frames where a nearly real pair of roots falls one way
    /// here and the other in Praat (a broad 100 Hz "formant" or none),
    /// which the resampling method alone can tip.
    #[test]
    fn formants_match_praat_on_a_real_voice() {
        let wav = Path::new(env!("CARGO_MANIFEST_DIR")).join("tutorial/counting-to-six.wav");
        let script = "import sys, math, parselmouth\ns = parselmouth.Sound(sys.argv[1])\nf = s.to_formant_burg(time_step=0.01, max_number_of_formants=5, maximum_formant=5500, window_length=0.025, pre_emphasis_from=50)\ni = s.to_intensity(minimum_pitch=75)\ntop = max(v for v in i.values[0] if not math.isnan(v))\nfor k in range(1, f.get_number_of_frames() + 1):\n    t = f.get_time_from_frame_number(k)\n    if t > 0.1 and t < s.duration - 0.1 and i.get_value(t) > top - 30:\n        print(t, *(f.get_value_at_time(n, t) for n in (1, 2, 3)))\n";
        let Ok(out) = std::process::Command::new("python3").arg("-c").arg(script).arg(&wav).output() else { return };
        if !out.status.success() {
            eprintln!("parselmouth is not installed; skipped");
            return;
        }
        let rows: Vec<Vec<f64>> = String::from_utf8_lossy(&out.stdout).lines().map(|l| l.split(' ').map(|x| x.parse().unwrap_or(f64::NAN)).collect()).collect();
        let times: Vec<f64> = rows.iter().map(|r| r[0]).collect();
        let (samples, rate, _) = read(&wav, 0.0, 6000.0).unwrap();
        let ours = formants(&samples, rate, &times);
        assert!(rows.len() > 200, "{} loud frames", rows.len());
        for k in 0..3 {
            let mut err: Vec<f64> = rows.iter().zip(&ours).filter_map(|(r, o)| Some((o[k]? - r[k + 1]).abs() / r[k + 1]).filter(|e| e.is_finite())).collect();
            err.sort_by(f64::total_cmp);
            let q = |p: f64| err[((err.len() - 1) as f64 * p) as usize];
            assert!(err.len() * 10 > rows.len() * 9, "F{}: {} of {} frames", k + 1, err.len(), rows.len());
            assert!(q(0.5) < 0.005 && q(0.9) < 0.03, "F{}: median {} p90 {}", k + 1, q(0.5), q(0.9));
        }
    }
}
