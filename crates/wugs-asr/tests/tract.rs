//! tract runs the wav2vec2 architecture the downloadable phone models use.
//! The fixtures are Hugging Face's Wav2Vec2ForCTC (large-model style),
//! shrunk, randomly initialized and exported with torch.onnx at opset 17
//! with a dynamic length, as the published exports are; `fixtures/
//! make_tiny_wav2vec2.py` rebuilds them. PyTorch's logits are the reference.

use std::path::PathBuf;

use wugs_asr::{Recognizer, Vocab};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn vocab() -> Vocab {
    Vocab::from_hf_json(&std::fs::read_to_string(fixture("tiny-wav2vec2.vocab.json")).unwrap()).unwrap()
}

fn audio() -> Vec<f32> {
    std::fs::read(fixture("tiny-wav2vec2.audio.f32")).unwrap().chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
}

fn reference() -> Vec<Vec<f32>> {
    std::fs::read_to_string(fixture("tiny-wav2vec2.logits.txt"))
        .unwrap()
        .lines()
        .map(|l| l.split_whitespace().map(|v| v.parse().unwrap()).collect())
        .collect()
}

fn max_diff(onnx: &str) -> (usize, f32, usize) {
    let r = Recognizer::load(&fixture(onnx), vocab(), true).unwrap();
    let (frames, v, logits) = r.logits(&audio()).unwrap();
    let want = reference();
    assert_eq!((frames, v), (want.len(), want[0].len()), "frames × vocab");
    let mut worst = 0f32;
    let mut argmax_agree = 0;
    for (t, row) in want.iter().enumerate() {
        let got = &logits[t * v..(t + 1) * v];
        for (a, b) in got.iter().zip(row) {
            worst = worst.max((a - b).abs());
        }
        let best = |r: &[f32]| r.iter().enumerate().fold((0, f32::MIN), |m, (i, &x)| if x > m.1 { (i, x) } else { m }).0;
        if best(got) == best(row) {
            argmax_agree += 1;
        }
    }
    (frames, worst, argmax_agree)
}

#[test]
fn the_fp32_export_matches_pytorch() {
    let (frames, worst, agree) = max_diff("tiny-wav2vec2.onnx");
    eprintln!("fp32: largest logit difference {worst}, {agree} of {frames} frames agree");
    assert!(worst < 1e-3, "largest logit difference {worst}");
    assert_eq!(agree, frames);
}

#[test]
fn the_fp16_export_runs_widened_to_f32() {
    let (frames, worst, agree) = max_diff("tiny-wav2vec2.fp16.onnx");
    eprintln!("fp16: largest logit difference {worst}, {agree} of {frames} frames agree");
    assert!(worst < 5e-2, "largest logit difference {worst}");
    assert!(agree as f32 >= frames as f32 * 0.95, "{agree} of {frames} frames pick the same symbol");
}

#[test]
fn any_length_runs_at_fifty_frames_a_second() {
    let r = Recognizer::load(&fixture("tiny-wav2vec2.onnx"), vocab(), true).unwrap();
    let a = audio();
    for seconds in [0.4f32, 1.0, 1.3] {
        let n = (16_000.0 * seconds) as usize;
        let (frames, _, _) = r.logits(&a[..n.min(a.len())]).unwrap();
        let expect = (seconds * 50.0) as i64;
        assert!((frames as i64 - expect).abs() <= 1, "{seconds} s gave {frames} frames");
        let rec = r.recognize(&a[..n.min(a.len())]).unwrap();
        assert!(rec.phones.iter().all(|p| p.end_ms <= seconds as f64 * 1000.0 + 25.0));
    }
}

#[test]
fn a_vocabulary_that_does_not_fit_the_model_is_refused() {
    let small = Vocab::from_hf_json(r#"{"<pad>": 0, "a": 1}"#).unwrap();
    let r = Recognizer::load(&fixture("tiny-wav2vec2.onnx"), small, true).unwrap();
    let err = r.logits(&audio()).unwrap_err().to_string();
    assert!(err.contains("vocabulary"), "{err}");
}
