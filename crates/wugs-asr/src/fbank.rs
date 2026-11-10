//! Kaldi-style log mel filterbank features, computed the way lhotse's
//! `Fbank(FbankConfig(num_filters=80, dither=0.0, snip_edges=False))` does,
//! which is what ZIPA and other icefall models were trained on: 25 ms
//! frames every 10 ms, edges reflected, DC offset removed, pre-emphasis
//! 0.97, a Povey window, a 512-point power spectrum, 80 triangular mel
//! bins from 20 Hz to Nyquist minus 400 Hz, natural log with a floor.
//! `tests/zipformer.rs` checks it against lhotse's own output.

use std::sync::Arc;

use realfft::{RealFftPlanner, RealToComplex};

pub const BINS: usize = 80;
const RATE: f64 = 16_000.0;
const FRAME: usize = 400;
const SHIFT: usize = 160;
const FFT: usize = 512;
const PREEMPH: f32 = 0.97;
const LOW_HZ: f64 = 20.0;
const HIGH_HZ: f64 = -400.0;

/// The features of 16 kHz mono audio in [-1, 1]: `(frames, frames × 80)`,
/// row-major. No frames for audio shorter than half a frame shift.
pub struct Fbank {
    window: Vec<f32>,
    /// Per mel bin: the first FFT bin it covers and its weights from there.
    banks: Vec<(usize, Vec<f32>)>,
    fft: Arc<dyn RealToComplex<f32>>,
}

impl Default for Fbank {
    fn default() -> Self {
        Fbank::new()
    }
}

impl Fbank {
    pub fn new() -> Fbank {
        // Povey window: a symmetric Hann window raised to 0.85.
        let window = (0..FRAME)
            .map(|i| (0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (FRAME - 1) as f64).cos()).powf(0.85) as f32)
            .collect();
        Fbank { window, banks: mel_banks(), fft: RealFftPlanner::<f32>::new().plan_fft_forward(FFT) }
    }

    pub fn compute(&self, audio: &[f32]) -> (usize, Vec<f32>) {
        let n = audio.len();
        let frames = (n + SHIFT / 2) / SHIFT;
        if frames == 0 {
            return (0, Vec::new());
        }
        // Reflect the edges as lhotse does (`snip_edges=False`): the first
        // `pad_left` samples mirrored before, the tail mirrored after.
        let pad_left = (FRAME - SHIFT) / 2;
        let at = |i: isize| -> f32 {
            let len = n as isize;
            if i < 0 {
                audio[((-i - 1) as usize).min(n - 1)]
            } else if i >= len {
                audio[(2 * len - 1 - i).max(0) as usize]
            } else {
                audio[i as usize]
            }
        };
        let mut out = Vec::with_capacity(frames * BINS);
        let mut buf = vec![0f32; FFT];
        let mut spec = self.fft.make_output_vec();
        let mut power = vec![0f32; FFT / 2 + 1];
        for f in 0..frames {
            let start = (f * SHIFT) as isize - pad_left as isize;
            let frame: Vec<f32> = (0..FRAME as isize).map(|k| at(start + k)).collect();
            let mean = frame.iter().sum::<f32>() / FRAME as f32;
            for k in 0..FRAME {
                let x = frame[k] - mean;
                let prev = if k == 0 { x } else { frame[k - 1] - mean };
                buf[k] = (x - PREEMPH * prev) * self.window[k];
            }
            buf[FRAME..].iter_mut().for_each(|v| *v = 0.0);
            self.fft.process(&mut buf, &mut spec).expect("fft sizes are fixed");
            for (p, c) in power.iter_mut().zip(&spec) {
                *p = c.norm_sqr();
            }
            for (first, weights) in &self.banks {
                let e: f32 = weights.iter().zip(&power[*first..]).map(|(w, p)| w * p).sum();
                out.push(e.max(f32::EPSILON).ln());
            }
        }
        (frames, out)
    }
}

fn mel(hz: f64) -> f64 {
    1127.0 * (1.0 + hz / 700.0).ln()
}

/// Triangular mel bins over the first FFT/2 bins (the Nyquist bin gets no
/// weight), as torchaudio's and lhotse's `get_mel_banks`.
fn mel_banks() -> Vec<(usize, Vec<f32>)> {
    let nyquist = RATE / 2.0;
    let high = if HIGH_HZ <= 0.0 { HIGH_HZ + nyquist } else { HIGH_HZ };
    let (lo, hi) = (mel(LOW_HZ), mel(high));
    let delta = (hi - lo) / (BINS + 1) as f64;
    let bin_hz = RATE / FFT as f64;
    (0..BINS)
        .map(|b| {
            let (left, center, right) = (lo + b as f64 * delta, lo + (b + 1) as f64 * delta, lo + (b + 2) as f64 * delta);
            let w: Vec<f32> = (0..FFT / 2)
                .map(|k| {
                    let m = mel(bin_hz * k as f64);
                    ((m - left) / (center - left)).min((right - m) / (right - center)).max(0.0) as f32
                })
                .collect();
            let first = w.iter().position(|&v| v > 0.0).unwrap_or(0);
            let last = w.iter().rposition(|&v| v > 0.0).map_or(first, |i| i + 1);
            (first, w[first..last].to_vec())
        })
        .collect()
}
