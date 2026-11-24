//! Spectrograms for the Timeline: what a phonetician reads formants,
//! fricatives and closures from, drawn under the waveform without Praat.
//! Computed here from the WAV in the blob store, ten seconds at a time,
//! with Praat's default broadband settings as the model (5 ms Gaussian
//! window, 0–5000 Hz, 6 dB/octave pre-emphasis), so the picture reads the way
//! linguists know it. `src/timeline/spectro.ts` draws it.
//!
//! A chunk goes to the webview as raw bytes: column after column (one per
//! STEP_MS), each ROWS values from 0 Hz up, each the power in half-dB steps
//! above -120 dB (`(dB + 120) * 2`, clamped to 0–255). The constants are
//! mirrored in the TypeScript.

use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::path::Path;

use realfft::RealFftPlanner;

/// One chunk's span, as the Praat overlays use.
pub const CHUNK_MS: u32 = 10_000;
/// Time between columns.
pub const STEP_MS: f64 = 4.0;
/// Rows from 0 to MAX_HZ, each ROW_HZ wide.
pub const ROWS: usize = 100;
pub const MAX_HZ: f64 = 5000.0;
/// Praat's broadband window: a Gaussian of effective length 5 ms, which
/// spans twice that; formants show as bands.
const WINDOW_MS: f64 = 10.0;
/// A narrowband window, as Praat users set it for pitch: effective length
/// 30 ms, so the harmonics of the voice show as lines (tone and intonation).
const NARROW_WINDOW_MS: f64 = 60.0;
/// Pre-emphasis starts here (6 dB per octave above it), as in Praat.
const EMPHASIS_FROM_HZ: f64 = 50.0;

/// The samples of `start_ms..end_ms` mixed to mono in -1..1 (silence past
/// either end), with the rate.
pub(crate) fn read_mono(wav: &Path, start_ms: f64, end_ms: f64) -> Result<(Vec<f32>, u32), String> {
    let mut f = File::open(wav).map_err(|e| e.to_string())?;
    let (fmt, frames) = {
        let r = wugs_peaks::WavReader::new(&mut f).map_err(|e| e.to_string())?;
        (r.format(), r.declared_frames())
    };
    let data = f.stream_position().map_err(|e| e.to_string())?;
    let rate = f64::from(fmt.sample_rate);
    let want_first = (start_ms / 1000.0 * rate).floor() as i64;
    let want_last = (end_ms / 1000.0 * rate).ceil() as i64;
    let len = (want_last - want_first).max(0) as usize;
    let mut out = vec![0f32; len];
    let first = want_first.max(0) as u64;
    let mut last = want_last.max(0) as u64;
    if let Some(n) = frames {
        last = last.min(n);
    }
    if last <= first {
        return Ok((out, fmt.sample_rate));
    }
    let align = fmt.block_align() as u64;
    f.seek(SeekFrom::Start(data + first * align)).map_err(|e| e.to_string())?;
    let mut reader = wugs_peaks::WavReader::resume(&mut f, fmt, Some((last - first) * align));
    let channels = usize::from(fmt.channels);
    let mut buf = vec![0i16; 4096 * channels];
    let mut at = (first as i64 - want_first) as usize;
    loop {
        let n = reader.read_frames(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        for frame in buf[..n * channels].chunks_exact(channels) {
            if at >= out.len() {
                break;
            }
            let sum: f32 = frame.iter().map(|&s| f32::from(s)).sum();
            out[at] = sum / (channels as f32 * 32768.0);
            at += 1;
        }
    }
    Ok((out, fmt.sample_rate))
}

/// The spectrogram of chunk `index` of `wav` (see the module comment for
/// the layout), broadband or, when `narrow`, narrowband.
pub fn chunk(wav: &Path, index: u32, narrow: bool) -> Result<Vec<u8>, String> {
    let window_ms = if narrow { NARROW_WINDOW_MS } else { WINDOW_MS };
    let start = f64::from(index) * f64::from(CHUNK_MS);
    let half = window_ms / 2.0;
    let (samples, rate) = read_mono(wav, start - half, start + f64::from(CHUNK_MS) + half)?;
    let rate_f = f64::from(rate);
    let win = ((window_ms / 1000.0) * rate_f).round().max(8.0) as usize;
    let size = win.next_power_of_two().max(256);
    // Gaussian window as Praat makes it (edges at about -26 dB).
    let window: Vec<f32> = (0..win)
        .map(|i| {
            let x = (i as f64 + 0.5) / win as f64 - 0.5;
            (-(x * x) * 12.0f64).exp() as f32
        })
        .collect();
    let gain: f64 = window.iter().map(|&w| f64::from(w)).sum::<f64>().powi(2);
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(size);
    let mut input = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let bin_hz = rate_f / size as f64;
    let columns = (f64::from(CHUNK_MS) / STEP_MS) as usize;
    let mut out = vec![0u8; columns * ROWS];
    let row_hz = MAX_HZ / ROWS as f64;
    let mut bins = vec![0f64; spectrum.len()];
    for c in 0..columns {
        // Window centre at the column's middle; sample 0 is `start - half`.
        let centre_ms = half + (c as f64 + 0.5) * STEP_MS;
        let first = ((centre_ms / 1000.0) * rate_f).round() as i64 - (win / 2) as i64;
        input.iter_mut().for_each(|v| *v = 0.0);
        for (i, w) in window.iter().enumerate() {
            let at = first + i as i64;
            if at >= 0 && (at as usize) < samples.len() {
                input[i] = samples[at as usize] * w;
            }
        }
        fft.process(&mut input, &mut spectrum).map_err(|e| e.to_string())?;
        for (b, v) in spectrum.iter().enumerate() {
            let hz = b as f64 * bin_hz;
            let mut power = f64::from(v.norm_sqr()) * 2.0 / gain;
            if hz > EMPHASIS_FROM_HZ {
                power *= (hz / EMPHASIS_FROM_HZ).powi(2);
            }
            bins[b] = 10.0 * (power + 1e-20).log10();
        }
        // Each row reads the spectrum at its middle frequency, between the
        // two nearest bins, so rows narrower than a bin still differ.
        for r in 0..ROWS {
            let at = (r as f64 + 0.5) * row_hz / bin_hz;
            let lo = (at.floor() as usize).min(bins.len() - 1);
            let hi = (lo + 1).min(bins.len() - 1);
            let db = bins[lo] + (bins[hi] - bins[lo]) * (at - lo as f64);
            out[c * ROWS + r] = ((db + 120.0) * 2.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mono 16-bit WAV of `secs` seconds: a sine at `hz` for the first half, silence after.
    fn tone(path: &Path, rate: u32, secs: f64, hz: f64) {
        let n = (rate as f64 * secs) as usize;
        let samples: Vec<i16> = (0..n)
            .map(|i| if i < n / 2 { ((i as f64 / rate as f64 * hz * std::f64::consts::TAU).sin() * 16000.0) as i16 } else { 0 })
            .collect();
        write_wav(path, rate, samples);
    }

    fn write_wav(path: &Path, rate: u32, samples: Vec<i16>) {
        let bytes = (samples.len() * 2) as u32;
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + bytes).to_le_bytes());
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
    fn narrowband_shows_the_harmonics_broadband_smears() {
        let dir = std::env::temp_dir().join(format!("wugs-spectro-n-{}", wugs_model::ulid()));
        std::fs::create_dir_all(&dir).unwrap();
        let rate = 16_000u32;
        let wav = dir.join("voice.wav");
        // A voice at 225 Hz: its first eight harmonics, equally loud.
        let samples = (0..rate as usize).map(|i| ((1..=8).map(|k| (i as f64 / f64::from(rate) * 225.0 * f64::from(k) * std::f64::consts::TAU).sin()).sum::<f64>() * 3000.0) as i16).collect();
        write_wav(&wav, rate, samples);
        let col = (500.0 / STEP_MS) as usize;
        // Row 4 is centred on 225 Hz, a harmonic; row 6 on 325 Hz, between two.
        let contrast = |narrow: bool| {
            let data = chunk(&wav, 0, narrow).unwrap();
            i32::from(data[col * ROWS + 4]) - i32::from(data[col * ROWS + 6])
        };
        let (narrow, broad) = (contrast(true), contrast(false));
        assert!(narrow > 40, "narrowband: a harmonic 20 dB above the gap, got {narrow} half-dB");
        assert!(narrow > broad + 20, "broadband smears the gap: {broad} against {narrow}");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_tone_shows_in_its_row_while_it_sounds() {
        let dir = std::env::temp_dir().join(format!("wugs-spectro-{}", wugs_model::ulid()));
        std::fs::create_dir_all(&dir).unwrap();
        for rate in [16_000u32, 44_100] {
            let wav = dir.join(format!("tone-{rate}.wav"));
            tone(&wav, rate, 2.0, 1000.0);
            let data = chunk(&wav, 0, false).unwrap();
            let columns = data.len() / ROWS;
            assert_eq!(columns, 2500);
            let at = |c: usize, r: usize| data[c * ROWS + r];
            // 0.5 s in: the 1000 Hz row is the loudest, far above 3000 Hz.
            let col = (500.0 / STEP_MS) as usize;
            let loudest = (0..ROWS).max_by_key(|&r| at(col, r)).unwrap();
            // Within 100 Hz of 1000 Hz (an FFT bin is up to 86 Hz wide, and
            // the pre-emphasis leans a peak upward).
            assert!((18..=22).contains(&loudest), "rate {rate}: row {loudest}");
            assert!(at(col, loudest) > at(col, 60) + 60, "30 dB above 3 kHz at {rate}: {} vs {}", at(col, loudest), at(col, 60));
            // 1.5 s in (silence) and past the end of the file: nothing.
            assert!(at((1500.0 / STEP_MS) as usize, loudest) < 10);
            assert!(at(columns - 1, loudest) < 10);
            // A chunk wholly past the end reads as silence, not an error.
            assert!(chunk(&wav, 3, false).unwrap().iter().all(|&v| v < 10));
        }
        std::fs::remove_dir_all(dir).ok();
    }
}
