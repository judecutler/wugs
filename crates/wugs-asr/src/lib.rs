//! Machine phone transcription for one segment at a time (roadmap item 27).
//!
//! A phone model is a CTC acoustic model in ONNX: mono 16 kHz audio in,
//! per-frame scores over a phone vocabulary out. This crate cuts a
//! segment's audio out of a WAV file, resamples it, runs the model with
//! `tract` (pure Rust, so nothing is downloaded at build time and every
//! platform builds the same), and decodes the scores into phones with a
//! confidence each. The model files themselves are an optional download;
//! nothing here ships with one.
//!
//! Two model families run here: [`Recognizer`] for models that take raw
//! audio (wav2vec2; the `onnx-ctc-raw` runtime) and
//! [`fbank_ctc::FbankRecognizer`] for models that take Kaldi filterbank
//! features (ZIPA's Zipformer; the `onnx-ctc-fbank` runtime).
//!
//! Edge crate: `tract-onnx`, `rubato`, `realfft` and `serde_json`.

pub mod fbank;
pub mod fbank_ctc;

use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::path::Path;

use rubato::audioadapter_buffers::owned::InterleavedOwned;
use rubato::{Fft, FixedSync, Resampler};
use tract_onnx::prelude::*;
use tract_onnx::tract_core::transform::{build_float_translator, NodeFilter};
use wugs_peaks::WavReader;

/// Every phone model this crate runs takes 16 kHz mono audio.
pub const SAMPLE_RATE: u32 = 16_000;

/// Segments longer than this are refused: attention cost grows with the
/// square of the length, and a suggestion is for one utterance.
pub const MAX_SECONDS: f64 = 30.0;

/// A gap of this many seconds of blank between two phones is written as a
/// space: the model knows no words, but a pause is the likeliest boundary.
pub const PAUSE_SECONDS: f64 = 0.25;

#[derive(Debug)]
pub enum Error {
    Audio(String),
    Model(String),
    Input(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Audio(m) => write!(f, "audio: {m}"),
            Error::Model(m) => write!(f, "model: {m}"),
            Error::Input(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

pub(crate) fn model_err(e: impl std::fmt::Display) -> Error {
    Error::Model(e.to_string())
}

/// One recognized phone: its symbol, where it sits in the segment, and the
/// model's probability for it, averaged over the frames that emitted it.
#[derive(Clone, Debug, PartialEq)]
pub struct Phone {
    pub symbol: String,
    pub start_ms: f64,
    pub end_ms: f64,
    pub confidence: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recognition {
    pub phones: Vec<Phone>,
    /// The mean of the phones' confidences; 0 when nothing was heard.
    pub confidence: f32,
}

impl Recognition {
    /// The phones as one IPA string: no spaces inside a stretch of speech,
    /// one space at each pause of at least [`PAUSE_SECONDS`].
    pub fn text(&self) -> String {
        let mut out = String::new();
        let mut last_end: Option<f64> = None;
        for p in &self.phones {
            if let Some(end) = last_end {
                if p.start_ms - end >= PAUSE_SECONDS * 1000.0 {
                    out.push(' ');
                }
            }
            out.push_str(&p.symbol);
            last_end = Some(p.end_ms);
        }
        out
    }
}

/// How to read a model's vocabulary and output.
#[derive(Clone, Debug)]
pub struct Vocab {
    /// Symbol per output index.
    pub symbols: Vec<String>,
    /// The CTC blank's index.
    pub blank: usize,
    /// Indices never emitted as phones (padding, unknown, sentence marks).
    pub skip: Vec<usize>,
}

impl Vocab {
    /// A Hugging Face `vocab.json`: `{ "<pad>": 0, "a": 5, … }`. The blank is
    /// `<pad>`; `<s>`, `</s>`, `<unk>` and the word separator `|` are never
    /// emitted as phones.
    pub fn from_hf_json(text: &str) -> Result<Vocab, Error> {
        let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(text).map_err(|e| Error::Model(format!("vocab.json: {e}")))?;
        let mut symbols = vec![String::new(); map.len()];
        for (sym, id) in &map {
            let id = id.as_u64().filter(|&i| (i as usize) < map.len()).ok_or_else(|| Error::Model(format!("vocab.json: {sym:?} has no usable index")))?;
            symbols[id as usize] = sym.clone();
        }
        let blank = symbols.iter().position(|s| s == "<pad>").ok_or_else(|| Error::Model("vocab.json has no <pad> (the CTC blank)".into()))?;
        let skip = symbols.iter().enumerate().filter(|(_, s)| matches!(s.as_str(), "<s>" | "</s>" | "<unk>" | "|")).map(|(i, _)| i).collect();
        Ok(Vocab { symbols, blank, skip })
    }
}

/// Greedy CTC decoding over per-frame logits (`frames × vocab`, row-major):
/// the best index per frame, repeats merged, blanks dropped. A phone's
/// confidence is the mean softmax probability of its frames.
pub fn ctc_greedy(logits: &[f32], vocab_len: usize, vocab: &Vocab, frame_ms: f64) -> Recognition {
    let mut phones: Vec<Phone> = Vec::new();
    // (index, first frame, last frame, probability sum, frame count)
    let mut run: Option<(usize, usize, usize, f32, u32)> = None;
    let close = |run: Option<(usize, usize, usize, f32, u32)>, phones: &mut Vec<Phone>| {
        if let Some((idx, first, last, sum, n)) = run {
            if idx != vocab.blank && !vocab.skip.contains(&idx) {
                phones.push(Phone {
                    symbol: vocab.symbols.get(idx).cloned().unwrap_or_default(),
                    start_ms: first as f64 * frame_ms,
                    end_ms: (last + 1) as f64 * frame_ms,
                    confidence: sum / n as f32,
                });
            }
        }
    };
    for (t, row) in logits.chunks_exact(vocab_len).enumerate() {
        let (best, max) = row.iter().enumerate().fold((0, f32::NEG_INFINITY), |acc, (i, &v)| if v > acc.1 { (i, v) } else { acc });
        let denom: f32 = row.iter().map(|&v| (v - max).exp()).sum();
        let p = 1.0 / denom;
        run = match run {
            Some((idx, first, _, sum, n)) if idx == best => Some((idx, first, t, sum + p, n + 1)),
            other => {
                close(other, &mut phones);
                Some((best, t, t, p, 1))
            }
        };
    }
    close(run, &mut phones);
    let confidence = if phones.is_empty() { 0.0 } else { phones.iter().map(|p| p.confidence).sum::<f32>() / phones.len() as f32 };
    Recognition { phones, confidence }
}

/// The span `[start_ms, end_ms)` of a WAV file as 16 kHz mono samples in
/// −1..1. The reader seeks to the span, so a segment an hour into a
/// recording costs no more than one at its start.
pub fn read_span(path: &Path, start_ms: f64, end_ms: f64) -> Result<Vec<f32>, Error> {
    if !(start_ms >= 0.0 && end_ms > start_ms) {
        return Err(Error::Input(format!("the segment has no length ({start_ms} to {end_ms} ms)")));
    }
    if (end_ms - start_ms) / 1000.0 > MAX_SECONDS {
        return Err(Error::Input(format!("the segment is longer than {MAX_SECONDS} s; split it to get a suggestion")));
    }
    let mut file = BufReader::new(File::open(path).map_err(|e| Error::Audio(format!("open {}: {e}", path.display())))?);
    let header = WavReader::new(&mut file).map_err(|e| Error::Audio(e.to_string()))?;
    let fmt = header.format();
    let data_frames = header.declared_frames();
    drop(header);
    let data_start = file.stream_position().map_err(|e| Error::Audio(e.to_string()))?;
    let rate = fmt.sample_rate as f64;
    let first = (start_ms / 1000.0 * rate).floor() as u64;
    let last = (end_ms / 1000.0 * rate).ceil() as u64;
    file.seek(SeekFrom::Start(data_start + first * fmt.block_align() as u64)).map_err(|e| Error::Audio(e.to_string()))?;
    let left = data_frames.map(|total| total.saturating_sub(first) * fmt.block_align() as u64);
    let mut reader = WavReader::resume(file, fmt, left);
    let ch = fmt.channels as usize;
    let want = (last - first) as usize;
    let mut mono = Vec::with_capacity(want);
    let mut buf = vec![0i16; 4096 * ch];
    while mono.len() < want {
        let n = reader.read_frames(&mut buf).map_err(|e| Error::Audio(e.to_string()))?;
        if n == 0 {
            break;
        }
        for frame in buf[..n * ch].chunks_exact(ch).take(want - mono.len()) {
            mono.push(frame.iter().map(|&s| s as f32).sum::<f32>() / (ch as f32 * 32768.0));
        }
    }
    if mono.is_empty() {
        return Err(Error::Input("the segment lies past the end of the recording".into()));
    }
    resample(&mono, fmt.sample_rate)
}

/// Mono audio at `rate` resampled to [`SAMPLE_RATE`].
pub fn resample(mono: &[f32], rate: u32) -> Result<Vec<f32>, Error> {
    if rate == SAMPLE_RATE {
        return Ok(mono.to_vec());
    }
    let mut r = Fft::<f32>::new(rate as usize, SAMPLE_RATE as usize, 1024, 1, FixedSync::Input).map_err(|e| Error::Audio(format!("resampler: {e}")))?;
    let input = InterleavedOwned::new_from(mono.to_vec(), 1, mono.len()).map_err(|e| Error::Audio(format!("resampler: {e}")))?;
    let out = r.process_all(&input, mono.len(), None).map_err(|e| Error::Audio(format!("resample: {e}")))?;
    Ok(out.take_data())
}

/// A loaded phone model, ready to run. Loading optimizes the whole graph,
/// which takes seconds for a large model, so keep one and reuse it.
pub struct Recognizer {
    plan: std::sync::Arc<TypedSimplePlan>,
    vocab: Vocab,
    /// Whether the model wants each clip normalized to zero mean and unit
    /// variance (wav2vec2's feature extractor does this).
    normalize: bool,
}

impl Recognizer {
    /// Load an ONNX CTC phone model whose one input is `(1, samples)` audio
    /// and whose first output is `(1, frames, vocab)` logits. Half-precision
    /// models are widened to f32, which every CPU runs.
    pub fn load(onnx: &Path, vocab: Vocab, normalize: bool) -> Result<Recognizer, Error> {
        let mut model = tract_onnx::onnx().model_for_path(onnx).map_err(model_err)?;
        let n = model.symbols.sym("N");
        let dt = model.input_fact(0).map_err(model_err)?.datum_type;
        let input_dt = if dt == DatumType::F16.into() { DatumType::F16 } else { DatumType::F32 };
        model.set_input_fact(0, InferenceFact::dt_shape(input_dt, tvec!(1.to_dim(), n.to_dim()))).map_err(model_err)?;
        let mut typed = model.into_typed().map_err(model_err)?;
        build_float_translator(DatumType::F16, DatumType::F32, NodeFilter::default()).transform(&mut typed).map_err(model_err)?;
        let plan = typed.into_optimized().map_err(model_err)?.into_runnable().map_err(model_err)?;
        Ok(Recognizer { plan, vocab, normalize })
    }

    /// Logits for 16 kHz mono audio, as `(frames, vocab_len, flat)`.
    pub fn logits(&self, audio: &[f32]) -> Result<(usize, usize, Vec<f32>), Error> {
        if audio.len() < SAMPLE_RATE as usize / 10 {
            return Err(Error::Input("the segment is shorter than a tenth of a second".into()));
        }
        let mut x = audio.to_vec();
        if self.normalize {
            let mean = x.iter().sum::<f32>() / x.len() as f32;
            let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / x.len() as f32;
            let scale = 1.0 / (var + 1e-7).sqrt();
            for v in &mut x {
                *v = (*v - mean) * scale;
            }
        }
        let input = tract_ndarray::Array2::from_shape_vec((1, x.len()), x).map_err(model_err)?.into_tensor();
        let out = self.plan.run(tvec!(input.into())).map_err(model_err)?;
        let logits = out[0].cast_to::<f32>().map_err(model_err)?.into_owned();
        let shape = logits.shape().to_vec();
        let (frames, vocab_len) = match shape.as_slice() {
            [1, t, v] => (*t, *v),
            [t, v] => (*t, *v),
            other => return Err(Error::Model(format!("expected (1, frames, vocab) logits, got {other:?}"))),
        };
        if vocab_len != self.vocab.symbols.len() {
            return Err(Error::Model(format!("the model scores {vocab_len} symbols but its vocabulary has {}", self.vocab.symbols.len())));
        }
        Ok((frames, vocab_len, logits.to_plain_array_view::<f32>().map_err(model_err)?.iter().copied().collect()))
    }

    /// Phones for 16 kHz mono audio.
    pub fn recognize(&self, audio: &[f32]) -> Result<Recognition, Error> {
        let (frames, vocab_len, logits) = self.logits(audio)?;
        let frame_ms = audio.len() as f64 / SAMPLE_RATE as f64 * 1000.0 / frames.max(1) as f64;
        Ok(ctc_greedy(&logits, vocab_len, &self.vocab, frame_ms))
    }
}
