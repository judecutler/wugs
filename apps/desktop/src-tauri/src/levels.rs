//! How well a recording was made: its peak and loudness, the room's noise
//! floor and the speech level above it, and where it clips (three or more
//! samples in a row at full scale, the sign of a gain set too high). What an
//! archive or a phonetician checks before trusting a recording; read once
//! through the whole WAV off the session thread, nothing cached or written.

use std::path::Path;

use serde::Serialize;

/// Analysis windows, in milliseconds.
const WINDOW_MS: f64 = 50.0;
/// Samples at or beyond this (of 32767) count as full scale.
const FULL: i32 = 32_766;
/// A clip is this many full-scale samples in a row on one channel.
const CLIP_RUN: u32 = 3;
/// Clip times listed, at most.
const MAX_LISTED: usize = 20;

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Levels {
    pub duration_ms: f64,
    pub sample_rate: u32,
    pub channels: u16,
    /// The loudest sample, in dB below full scale (0 is full scale); None for silence.
    pub peak_db: Option<f64>,
    /// The whole recording's RMS level, in dBFS.
    pub rms_db: Option<f64>,
    /// The quiet 10th percentile of 50 ms windows: the room's noise floor.
    pub floor_db: Option<f64>,
    /// The loud 95th percentile of 50 ms windows: the speech level.
    pub speech_db: Option<f64>,
    /// Clips (runs of full-scale samples) and where the first ones start, in ms.
    pub clips: u64,
    pub clip_times_ms: Vec<f64>,
}

fn to_db(amplitude: f64) -> Option<f64> {
    (amplitude > 0.0).then(|| 20.0 * (amplitude / 32768.0).log10())
}

fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    Some(sorted[i])
}

pub fn levels(path: &Path) -> Result<Levels, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut reader = wugs_peaks::WavReader::new(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
    let fmt = reader.format();
    let ch = fmt.channels.max(1) as usize;
    let window = ((fmt.sample_rate as f64 * WINDOW_MS / 1000.0).round() as usize).max(1);
    let mut buf = vec![0i16; 8192 * ch];
    let mut frames: u64 = 0;
    let mut peak: i32 = 0;
    let mut sum_sq: f64 = 0.0;
    let mut win_sq: f64 = 0.0;
    let mut win_n: usize = 0;
    let mut windows: Vec<f64> = Vec::new();
    let mut runs = vec![0u32; ch];
    let mut clips: u64 = 0;
    let mut clip_times: Vec<f64> = Vec::new();
    loop {
        let n = reader.read_frames(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        for f in 0..n {
            let mut frame_sq = 0.0;
            for c in 0..ch {
                let v = buf[f * ch + c] as i32;
                let a = v.abs();
                peak = peak.max(a);
                frame_sq += (v as f64) * (v as f64);
                if a >= FULL {
                    runs[c] += 1;
                    if runs[c] == CLIP_RUN {
                        clips += 1;
                        if clip_times.len() < MAX_LISTED {
                            clip_times.push((frames + f as u64) as f64 * 1000.0 / fmt.sample_rate as f64);
                        }
                    }
                } else {
                    runs[c] = 0;
                }
            }
            let mean = frame_sq / ch as f64;
            sum_sq += mean;
            win_sq += mean;
            win_n += 1;
            if win_n == window {
                windows.push((win_sq / win_n as f64).sqrt());
                win_sq = 0.0;
                win_n = 0;
            }
        }
        frames += n as u64;
    }
    // A window holding only digital silence says nothing about the room.
    let mut heard: Vec<f64> = windows.into_iter().filter(|w| *w >= 1.0).filter_map(to_db).collect();
    heard.sort_by(f64::total_cmp);
    Ok(Levels {
        duration_ms: frames as f64 * 1000.0 / fmt.sample_rate.max(1) as f64,
        sample_rate: fmt.sample_rate,
        channels: fmt.channels,
        peak_db: to_db(peak as f64),
        rms_db: if frames == 0 { None } else { to_db((sum_sq / frames as f64).sqrt()) },
        floor_db: percentile(&heard, 0.10),
        speech_db: percentile(&heard, 0.95),
        clips,
        clip_times_ms: clip_times,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(path: &Path, rate: u32, samples: &[i16]) {
        let mut out = Vec::new();
        let data = samples.len() as u32 * 2;
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data.to_le_bytes());
        for s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(path, out).unwrap();
    }

    #[test]
    fn a_tone_over_hiss_with_two_clips() {
        let dir = std::env::temp_dir().join(format!("wugs-levels-{}", wugs_model::ulid()));
        std::fs::create_dir_all(&dir).unwrap();
        let rate = 16_000;
        let mut s: Vec<i16> = Vec::new();
        // One second of quiet hiss (±32, about −60 dBFS), one of a tone at half scale.
        let mut seed: u32 = 7;
        for _ in 0..rate {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            s.push(((seed >> 16) % 65) as i16 - 32);
        }
        for i in 0..rate {
            s.push((16_384.0 * (i as f64 * 440.0 * std::f64::consts::TAU / rate as f64).sin()) as i16);
        }
        // Two clips: five full-scale samples, then four at the negative rail.
        let at = s.len();
        s.extend([32_767; 5]);
        s.extend([0; 100]);
        s.extend([-32_768; 4]);
        s.extend([0; 100]);
        let path = dir.join("t.wav");
        wav(&path, rate, &s);
        let l = levels(&path).unwrap();
        assert_eq!(l.clips, 2, "{l:?}");
        assert!((l.clip_times_ms[0] - at as f64 * 1000.0 / rate as f64).abs() < 0.2, "{l:?}");
        assert!(l.peak_db.unwrap() > -0.01, "{l:?}");
        assert!((l.speech_db.unwrap() - -9.0).abs() < 1.0, "a half-scale sine is about −9 dBFS RMS: {l:?}");
        assert!(l.floor_db.unwrap() < -55.0, "{l:?}");
        assert!((l.duration_ms - (s.len() as f64 * 1000.0 / rate as f64)).abs() < 0.01);
        std::fs::remove_dir_all(&dir).ok();
    }
}
