//! The `onnx-ctc-fbank` runtime against reference implementations: the
//! filterbank against lhotse, and a tiny ZIPA-style Zipformer (made by
//! `fixtures/make_tiny_zipformer.py`) under tract against onnxruntime,
//! including a 30 s input that needs the longer positional table.

use std::path::PathBuf;

use wugs_asr::fbank::{Fbank, BINS};
use wugs_asr::fbank_ctc::{positional_table, FbankRecognizer};
use wugs_asr::Vocab;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn f32s(name: &str) -> Vec<f32> {
    std::fs::read(fixture(name)).unwrap().chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

/// The made-up features the fixture script feeds the model.
fn feats(t: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(t * BINS);
    for i in 0..t {
        for f in 0..BINS {
            let (i, f) = (i as f64, f as f64);
            out.push(((0.37 * i + 1.3 * f).sin() * 2.0 + (0.011 * i * (f + 1.0)).cos()) as f32);
        }
    }
    out
}

fn tokens() -> Vocab {
    let mut text = String::from("<blk> 0\n<sos/eos> 1\n<unk> 2\n▁ 3\n");
    for i in 4..127 {
        text.push_str(&format!("p{i} {i}\n"));
    }
    Vocab::from_tokens_txt(&text).unwrap()
}

#[test]
fn fbank_matches_lhotse() {
    let audio = f32s("fbank.audio.f32");
    let want = f32s("fbank.lhotse.f32");
    let (frames, got) = Fbank::new().compute(&audio);
    assert_eq!(frames * BINS, want.len());
    let worst = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
    assert!(worst < 1e-3, "largest difference from lhotse: {worst}");
}

#[test]
fn the_positional_table_has_zipformers_shape() {
    let pe = positional_table(3, 4);
    assert_eq!(pe.len(), 5 * 4);
    // The middle row is position 0: cos 0 = 1, sin 0 = 0, and the bias column.
    assert_eq!(&pe[2 * 4..3 * 4], &[1.0, 0.0, 1.0, 1.0]);
}

fn best(row: &[f32]) -> usize {
    row.iter().enumerate().fold((0, f32::MIN), |m, (i, &x)| if x > m.1 { (i, x) } else { m }).0
}

#[test]
fn tract_runs_the_zipformer_as_onnxruntime_does() {
    let r = FbankRecognizer::load(&fixture("tiny-zipformer.onnx"), tokens()).unwrap();
    let (t, v, got) = r.log_probs_for_features(173, feats(173)).unwrap();
    assert_eq!((t, v), (83, 127));
    let want = f32s("tiny-zipformer.173.out.f32");
    let worst = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
    assert!(worst < 1e-4, "largest difference from onnxruntime: {worst}");

    // 30 s: past the exported positional table (1000 frames), so this runs
    // on the table computed at load.
    let (t, _, got) = r.log_probs_for_features(3000, feats(3000)).unwrap();
    assert_eq!(t, 1496);
    let want = f32s("tiny-zipformer.3000.out.f32");
    let ends: Vec<f32> = got[..40 * v].iter().chain(&got[(t - 40) * v..]).copied().collect();
    let worst = ends.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
    assert!(worst < 1e-3, "largest difference from onnxruntime at 30 s: {worst}");
    assert!(r.log_probs_for_features(3001, feats(3001)).is_err(), "past 30 s is refused");
    assert!(r.log_probs_for_features(10, feats(10)).is_err(), "too short is refused");
}

#[test]
fn an_unoptimized_plan_gives_the_same_answer() {
    let r = FbankRecognizer::load(&fixture("tiny-zipformer.onnx"), tokens()).unwrap().without_optimizing();
    let (_, _, got) = r.log_probs_for_features(173, feats(173)).unwrap();
    let want = f32s("tiny-zipformer.173.out.f32");
    let worst = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
    assert!(worst < 1e-4, "largest difference from onnxruntime: {worst}");
}

#[test]
fn recognizing_audio_runs_end_to_end() {
    let r = FbankRecognizer::load(&fixture("tiny-zipformer.onnx"), tokens()).unwrap();
    let audio = f32s("fbank.audio.f32");
    let rec = r.recognize(&audio).unwrap();
    // A random model hears nonsense; what matters is that it hears something
    // shaped like phones, with times inside the clip.
    assert!(rec.phones.iter().all(|p| p.end_ms <= 1230.0 + 1e-6 && (0.0..=1.0).contains(&p.confidence)));
    assert!(r.recognize(&audio[..800]).is_err());
}

#[test]
fn the_int8_release_format_runs_too() {
    // ZIPA's INT8 files quantize every MatMul (DynamicQuantizeLinear +
    // MatMulInteger); quantization rounds a little differently in tract.
    let r = FbankRecognizer::load(&fixture("tiny-zipformer.int8.onnx"), tokens()).unwrap();
    let (t, v, got) = r.log_probs_for_features(173, feats(173)).unwrap();
    let want = f32s("tiny-zipformer.int8.173.out.f32");
    let worst = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
    assert!(worst < 0.05, "largest difference from onnxruntime: {worst}");
    let agree = got.chunks(v).zip(want.chunks(v)).filter(|(a, b)| best(a) == best(b)).count();
    assert!(agree * 100 >= t * 95, "best symbol agrees on {agree} of {t} frames");
}

/// The real ZIPA release under tract against onnxruntime. Needs the files
/// the `real-models` workflow downloads: `WUGS_ZIPA_DIR` holding
/// `model.int8.onnx`, `tokens.txt` and `ref.<T>.out.f32` from
/// `fixtures/real_zipa_reference.py`.
#[test]
#[ignore]
fn the_real_zipa_release_matches_onnxruntime() {
    let dir = PathBuf::from(std::env::var("WUGS_ZIPA_DIR").expect("set WUGS_ZIPA_DIR"));
    let vocab = Vocab::from_tokens_txt(&std::fs::read_to_string(dir.join("tokens.txt")).unwrap()).unwrap();
    for optimize in [true, false] {
        let r = FbankRecognizer::load(&dir.join("model.int8.onnx"), vocab.clone()).unwrap();
        let r = if optimize { r } else { r.without_optimizing() };
        for t in [500usize, 3000] {
            let t0 = std::time::Instant::now();
            let (frames, v, got) = r.log_probs_for_features(t, feats(t)).unwrap();
            let first = t0.elapsed();
            let t0 = std::time::Instant::now();
            r.log_probs_for_features(t, feats(t)).unwrap();
            eprintln!("optimize={optimize}: {t} input frames, first run {first:?} (plan built), again {:?}", t0.elapsed());
            let want: Vec<f32> = std::fs::read(dir.join(format!("ref.{t}.out.f32"))).unwrap().chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            assert_eq!(got.len(), want.len());
            let agree = got.chunks(v).zip(want.chunks(v)).filter(|(a, b)| best(a) == best(b)).count();
            assert!(agree * 100 >= frames * 99, "best symbol agrees on only {agree} of {frames} frames");
        }
    }
}

/// End to end on real speech: the whole wugs pipeline (WAV reading,
/// filterbank, the adjusted graph under tract) against ZIPA's own
/// inference script (lhotse + onnxruntime) on the sample recording in
/// ZIPA's repository. Needs `WUGS_ZIPA_DIR` as above plus `sample.wav`
/// and `sample.expected.txt` (the script's space-separated phones).
#[test]
#[ignore]
fn the_real_zipa_release_transcribes_like_zipas_own_script() {
    let dir = PathBuf::from(std::env::var("WUGS_ZIPA_DIR").expect("set WUGS_ZIPA_DIR"));
    let vocab = Vocab::from_tokens_txt(&std::fs::read_to_string(dir.join("tokens.txt")).unwrap()).unwrap();
    let audio = wugs_asr::read_span(&dir.join("sample.wav"), 0.0, 30_000.0).unwrap();
    let want: Vec<String> = std::fs::read_to_string(dir.join("sample.expected.txt"))
        .unwrap()
        .split_whitespace()
        .filter(|s| !matches!(*s, "▁" | "<unk>" | "<sos/eos>"))
        .map(str::to_string)
        .collect();
    let r = FbankRecognizer::load(&dir.join("model.int8.onnx"), vocab).unwrap();
    let got: Vec<String> = r.recognize(&audio).unwrap().phones.into_iter().map(|p| p.symbol).collect();
    eprintln!("ours:   {}\nZIPA's: {}", got.join(" "), want.join(" "));
    let distance = edits(&got, &want);
    eprintln!("{distance} edits over {} phones", want.len());
    assert!(!want.is_empty() && distance * 100 <= want.len() * 3, "{distance} edits over {} phones", want.len());
}

fn edits(got: &[String], want: &[String]) -> usize {
    // Levenshtein distance over phones.
    let mut row: Vec<usize> = (0..=want.len()).collect();
    for (i, a) in got.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, b) in want.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(a != b)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[want.len()]
}
