use std::io::Write;

use wugs_asr::{ctc_greedy, read_span, resample, Phone, Recognition, Vocab, SAMPLE_RATE};

fn vocab() -> Vocab {
    Vocab::from_hf_json(r#"{"<pad>": 0, "<s>": 1, "</s>": 2, "<unk>": 3, "|": 4, "a": 5, "b": 6, "ʃ": 7}"#).unwrap()
}

/// One frame of logits that strongly prefers `idx` (probability ≈ p).
fn frame(idx: usize, p: f32) -> Vec<f32> {
    let n = 8;
    let other = (1.0 - p) / (n - 1) as f32;
    (0..n).map(|i| if i == idx { p.ln() } else { other.ln() }).collect()
}

fn logits(frames: &[(usize, f32)]) -> Vec<f32> {
    frames.iter().flat_map(|&(i, p)| frame(i, p)).collect()
}

#[test]
fn vocab_reads_hugging_face_json() {
    let v = vocab();
    assert_eq!(v.blank, 0);
    assert_eq!(v.symbols[7], "ʃ");
    assert_eq!(v.skip, vec![1, 2, 3, 4]);
    assert!(Vocab::from_hf_json(r#"{"a": 0}"#).is_err(), "no <pad> blank");
    assert!(Vocab::from_hf_json(r#"{"<pad>": 0, "a": 9}"#).is_err(), "index past the end");
}

#[test]
fn greedy_decoding_merges_repeats_and_drops_blanks() {
    // a a _ a b b _ ʃ : "a" twice (blank between), "b", "ʃ"
    let l = logits(&[(5, 0.9), (5, 0.7), (0, 0.9), (5, 0.6), (6, 0.8), (6, 0.8), (0, 0.9), (7, 0.5)]);
    let r = ctc_greedy(&l, 8, &vocab(), 20.0);
    let syms: Vec<_> = r.phones.iter().map(|p| p.symbol.as_str()).collect();
    assert_eq!(syms, ["a", "a", "b", "ʃ"]);
    assert_eq!((r.phones[0].start_ms, r.phones[0].end_ms), (0.0, 40.0));
    assert!((r.phones[0].confidence - 0.8).abs() < 1e-4, "{}", r.phones[0].confidence);
    assert!((r.phones[3].confidence - 0.5).abs() < 1e-4);
    let mean = (0.8 + 0.6 + 0.8 + 0.5) / 4.0;
    assert!((r.confidence - mean).abs() < 1e-4, "{}", r.confidence);
}

#[test]
fn special_tokens_are_never_phones() {
    let r = ctc_greedy(&logits(&[(1, 0.9), (4, 0.9), (3, 0.9), (5, 0.9), (2, 0.9)]), 8, &vocab(), 20.0);
    assert_eq!(r.phones.len(), 1);
    assert_eq!(r.phones[0].symbol, "a");
}

#[test]
fn silence_gives_nothing_and_no_confidence() {
    let r = ctc_greedy(&logits(&[(0, 0.99); 10]), 8, &vocab(), 20.0);
    assert!(r.phones.is_empty());
    assert_eq!(r.confidence, 0.0);
    assert_eq!(r.text(), "");
}

#[test]
fn a_pause_becomes_a_space() {
    let p = |s: &str, a: f64, b: f64| Phone { symbol: s.into(), start_ms: a, end_ms: b, confidence: 1.0 };
    let r = Recognition { phones: vec![p("a", 0.0, 40.0), p("ʃ", 60.0, 100.0), p("b", 400.0, 440.0)], confidence: 1.0 };
    assert_eq!(r.text(), "aʃ b");
}

fn tone(rate: u32, hz: f32, seconds: f32) -> Vec<f32> {
    (0..(rate as f32 * seconds) as usize).map(|i| (i as f32 / rate as f32 * hz * std::f32::consts::TAU).sin() * 0.5).collect()
}

#[test]
fn resampling_keeps_length_and_pitch() {
    let x = tone(48_000, 440.0, 1.0);
    let y = resample(&x, 48_000).unwrap();
    assert!((y.len() as i64 - SAMPLE_RATE as i64).abs() < 50, "{}", y.len());
    // Count zero crossings in the middle half: a 440 Hz tone crosses 880 times a second.
    let mid = &y[y.len() / 4..3 * y.len() / 4];
    let crossings = mid.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
    assert!((crossings as i64 - 440).abs() <= 4, "{crossings}");
    assert_eq!(resample(&x[..100], SAMPLE_RATE).unwrap().len(), 100, "16 kHz passes through");
}

/// A 16-bit WAV of `channels` channels where every channel holds `f(t)`,
/// with a LIST chunk after the data so a reader that runs past the data
/// chunk would read it as audio.
fn wav(rate: u32, channels: u16, seconds: f32, f: impl Fn(f32) -> f32) -> tempfile_path::Path {
    let frames = (rate as f32 * seconds) as usize;
    let mut data = Vec::new();
    for i in 0..frames {
        let v = (f(i as f32 / rate as f32) * 32767.0) as i16;
        for _ in 0..channels {
            data.extend_from_slice(&v.to_le_bytes());
        }
    }
    let list = b"LIST\x10\x00\x00\x00INFOjunkjunkjunk";
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + data.len() + list.len()) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    out.extend_from_slice(list);
    let path = tempfile_path::Path::new();
    std::fs::File::create(&path.0).unwrap().write_all(&out).unwrap();
    path
}

mod tempfile_path {
    pub struct Path(pub std::path::PathBuf);
    impl Path {
        pub fn new() -> Path {
            static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Path(std::env::temp_dir().join(format!("wugs-asr-{}-{n}.wav", std::process::id())))
        }
    }
    impl Drop for Path {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}

#[test]
fn a_span_is_cut_from_the_middle_and_mixed_down() {
    // One second of silence, then one second at constant 0.5.
    let w = wav(16_000, 2, 2.0, |t| if t >= 1.0 { 0.5 } else { 0.0 });
    let span = read_span(&w.0, 1000.0, 1500.0).unwrap();
    assert_eq!(span.len(), 8000);
    assert!(span.iter().all(|&v| (v - 0.5).abs() < 0.01), "stereo mixed down to the channel value");
    let before = read_span(&w.0, 500.0, 900.0).unwrap();
    assert!(before.iter().all(|&v| v.abs() < 1e-6));
}

#[test]
fn a_span_past_the_data_never_reads_the_chunks_after_it() {
    let w = wav(16_000, 1, 1.0, |_| 0.25);
    let tail = read_span(&w.0, 900.0, 1400.0).unwrap();
    assert_eq!(tail.len(), 1600, "only the 100 ms that exist");
    assert!(tail.iter().all(|&v| (v - 0.25).abs() < 0.01), "no LIST bytes read as audio");
    assert!(read_span(&w.0, 1200.0, 1300.0).is_err(), "wholly past the end");
}

#[test]
fn spans_are_checked() {
    let w = wav(8_000, 1, 1.0, |_| 0.0);
    assert!(read_span(&w.0, 500.0, 500.0).is_err());
    assert!(read_span(&w.0, 0.0, 31_000.0).is_err());
    assert_eq!(read_span(&w.0, 0.0, 500.0).unwrap().len(), 8000, "8 kHz resampled to 16 kHz");
}
