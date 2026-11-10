//! The `onnx-ctc-fbank` runtime: CTC phone models that take 80-bin Kaldi
//! filterbank features, `x (1, T, 80)` and `x_lens (1,)`, and return
//! `log_probs (1, T', vocab)`. ZIPA's Zipformer CTC models are the first.
//!
//! tract cannot run icefall's Zipformer ONNX export as it comes, so the
//! graph is adjusted once at load, in ways that do not change what it
//! computes (`tests/zipformer.rs` checks the result against onnxruntime):
//!
//! - each relative positional encoding is an `If` that either returns a
//!   precomputed table (long enough for 1000 frames) or rebuilds a longer
//!   one; tract evaluates both branches, and the rebuild uses a negative
//!   ScatterND index it rejects. The `If` becomes a constant table long
//!   enough for `MAX_SECONDS`, computed with the same formula and checked
//!   against the exported table where they overlap.
//! - `Range` with a negative step (the attention's relative-to-absolute
//!   shift) gets the wrong length in tract's shape inference; it becomes
//!   `start - Range(0, start - limit, -step)`.
//! - the sequence length is made concrete per run (with `x_lens` as a
//!   constant), because tract cannot follow the mask's data-dependent
//!   shapes symbolically. Plans are cached per length.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tract_onnx::pb::{attribute_proto, tensor_proto, AttributeProto, GraphProto, ModelProto, NodeProto, TensorProto};
use tract_onnx::prelude::*;

use crate::fbank::{Fbank, BINS};
use crate::{ctc_greedy, model_err, Error, Recognition, Vocab, MAX_SECONDS, SAMPLE_RATE};

/// Optimized plans kept, by input length: a batch run over segments of
/// the same length reuses one; most suggestions differ in length.
const PLANS: usize = 4;

pub struct FbankRecognizer {
    proto: ModelProto,
    vocab: Vocab,
    fbank: Fbank,
    plans: Mutex<VecDeque<(usize, Arc<TypedSimplePlan>)>>,
}

impl Vocab {
    /// An icefall `tokens.txt`: one `symbol id` per line. `<blk>` is the CTC
    /// blank; `<sos/eos>`, `<unk>` and the word marker `▁` are not phones.
    pub fn from_tokens_txt(text: &str) -> Result<Vocab, Error> {
        let mut pairs = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let (sym, id) = line.rsplit_once(' ').ok_or_else(|| Error::Model(format!("tokens.txt line {}: expected `symbol id`", n + 1)))?;
            let id: usize = id.trim().parse().map_err(|_| Error::Model(format!("tokens.txt line {}: {id:?} is not an id", n + 1)))?;
            pairs.push((id, sym.to_string()));
        }
        let len = pairs.iter().map(|(i, _)| i + 1).max().unwrap_or(0);
        if len == 0 || len > 100_000 {
            return Err(Error::Model("tokens.txt is empty or implausibly large".into()));
        }
        let mut symbols = vec![String::new(); len];
        for (id, sym) in pairs {
            symbols[id] = sym;
        }
        let blank = symbols.iter().position(|s| s == "<blk>").ok_or_else(|| Error::Model("tokens.txt has no <blk> (the CTC blank)".into()))?;
        let skip = symbols.iter().enumerate().filter(|(_, s)| matches!(s.as_str(), "<sos/eos>" | "<unk>" | "▁" | "")).map(|(i, _)| i).collect();
        Ok(Vocab { symbols, blank, skip })
    }
}

impl FbankRecognizer {
    pub fn load(onnx: &Path, vocab: Vocab) -> Result<FbankRecognizer, Error> {
        let mut proto = tract_onnx::onnx().proto_model_for_path(onnx).map_err(model_err)?;
        let graph = proto.graph.as_mut().ok_or_else(|| Error::Model("the ONNX file has no graph".into()))?;
        // Long enough for the longest segment at the first stack's rate
        // (100 frames/s, halved by the subsampling), with room to spare.
        let max_frames = (MAX_SECONDS * 100.0 / 2.0) as usize + 64;
        inline_positional_tables(graph, max_frames)?;
        rewrite_negative_ranges(graph);
        // The exported shapes name a symbolic batch; ours is always 1.
        graph.value_info.clear();
        Ok(FbankRecognizer { proto, vocab, fbank: Fbank::new(), plans: Mutex::new(VecDeque::new()) })
    }

    fn plan(&self, frames: usize) -> Result<Arc<TypedSimplePlan>, Error> {
        if let Some((_, p)) = self.plans.lock().expect("plans lock").iter().find(|(n, _)| *n == frames) {
            return Ok(p.clone());
        }
        let mut model = tract_onnx::onnx().model_for_proto_model(&self.proto).map_err(model_err)?;
        model.set_input_fact(0, InferenceFact::dt_shape(DatumType::F32, tvec!(1.to_dim(), frames.to_dim(), BINS.to_dim()))).map_err(model_err)?;
        model.set_input_fact(1, InferenceFact::from(tract_ndarray::arr1(&[frames as i64]).into_tensor())).map_err(model_err)?;
        let x = model.node(model.input_outlets().map_err(model_err)?[0].node).name.clone();
        let model = model.with_input_names([x]).map_err(model_err)?;
        let plan = model.into_typed().map_err(model_err)?.into_optimized().map_err(model_err)?.into_runnable().map_err(model_err)?;
        let mut plans = self.plans.lock().expect("plans lock");
        plans.push_front((frames, plan.clone()));
        plans.truncate(PLANS);
        Ok(plan)
    }

    /// Log probabilities for filterbank features, as `(frames, vocab_len, flat)`.
    pub fn log_probs_for_features(&self, frames: usize, feats: Vec<f32>) -> Result<(usize, usize, Vec<f32>), Error> {
        let plan = self.plan(frames)?;
        let x = tract_ndarray::Array3::from_shape_vec((1, frames, BINS), feats).map_err(model_err)?.into_tensor();
        let out = plan.run(tvec!(x.into())).map_err(model_err)?;
        let lp = out[0].cast_to::<f32>().map_err(model_err)?.into_owned();
        let (t, v) = match lp.shape() {
            [1, t, v] => (*t, *v),
            other => return Err(Error::Model(format!("expected (1, frames, vocab) log probabilities, got {other:?}"))),
        };
        if v != self.vocab.symbols.len() {
            return Err(Error::Model(format!("the model scores {v} symbols but tokens.txt lists {}", self.vocab.symbols.len())));
        }
        Ok((t, v, lp.to_plain_array_view::<f32>().map_err(model_err)?.iter().copied().collect()))
    }

    pub fn recognize(&self, audio: &[f32]) -> Result<Recognition, Error> {
        // Features need at least 8 frames to survive the subsampling.
        if audio.len() < SAMPLE_RATE as usize / 10 {
            return Err(Error::Input("the segment is shorter than a tenth of a second".into()));
        }
        let (frames, feats) = self.fbank.compute(audio);
        let (t, v, lp) = self.log_probs_for_features(frames, feats)?;
        let frame_ms = audio.len() as f64 / SAMPLE_RATE as f64 * 1000.0 / t.max(1) as f64;
        // ctc_greedy takes logits and softmaxes them; log probabilities are
        // logits whose softmax is the probabilities themselves.
        Ok(ctc_greedy(&lp, v, &self.vocab, frame_ms))
    }
}

/// Zipformer's `CompactRelPositionalEncoding` table for `t` positions each
/// side, `dim` wide (length_factor 1.0, as every Zipformer2 builds it).
pub fn positional_table(t: usize, dim: usize) -> Vec<f32> {
    let compression = (dim as f64).sqrt();
    let length_scale = dim as f64 / (2.0 * std::f64::consts::PI);
    let rows = 2 * t - 1;
    let mut pe = vec![0f32; rows * dim];
    for r in 0..rows {
        let x = r as f64 - (t as f64 - 1.0);
        let xc = compression * x.signum() * ((x.abs() + compression).ln() - compression.ln());
        // torch computes in f32 from here on; doing the same keeps the
        // table equal to the exported one.
        let atan = ((xc / length_scale) as f32).atan();
        for k in 0..dim / 2 {
            let a = atan * (k + 1) as f32;
            pe[r * dim + 2 * k] = a.cos();
            pe[r * dim + 2 * k + 1] = a.sin();
        }
        pe[r * dim + dim - 1] = 1.0;
    }
    pe
}

fn floats(t: &TensorProto) -> Option<Vec<f32>> {
    if t.data_type != tensor_proto::DataType::Float as i32 {
        return None;
    }
    if !t.float_data.is_empty() {
        return Some(t.float_data.clone());
    }
    (t.raw_data.len() % 4 == 0).then(|| t.raw_data.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
}

fn subgraph<'a>(n: &'a NodeProto, name: &str) -> Option<&'a GraphProto> {
    n.attribute.iter().find(|a| a.name == name).and_then(|a| a.g.as_ref())
}

/// Replace each positional-encoding `If` (a then-branch that is one 2-D
/// constant `(2T-1, dim)`) with a constant table for `max_t` positions.
fn inline_positional_tables(graph: &mut GraphProto, max_t: usize) -> Result<(), Error> {
    for node in graph.node.iter_mut().filter(|n| n.op_type == "If") {
        let Some(then) = subgraph(node, "then_branch") else { continue };
        let [only] = then.node.as_slice() else { continue };
        let Some(t) = (only.op_type == "Constant").then(|| only.attribute.iter().find(|a| a.name == "value")).flatten().and_then(|a| a.t.as_ref()) else {
            continue;
        };
        let [rows, dim] = t.dims[..] else { continue };
        let (rows, dim) = (rows as usize, dim as usize);
        let Some(exported) = floats(t) else { continue };
        if rows % 2 == 0 || dim % 2 != 0 || exported.len() != rows * dim {
            continue;
        }
        let ours = positional_table(rows.div_ceil(2), dim);
        let worst = ours.iter().zip(&exported).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        if worst > 1e-4 {
            return Err(Error::Model(format!("{}: the positional table is not Zipformer's (differs by {worst})", node.name)));
        }
        let t_out = max_t.max(rows.div_ceil(2));
        let table = positional_table(t_out, dim);
        let value = TensorProto {
            dims: vec![(2 * t_out - 1) as i64, dim as i64],
            data_type: tensor_proto::DataType::Float as i32,
            raw_data: table.iter().flat_map(|v| v.to_le_bytes()).collect(),
            ..Default::default()
        };
        *node = NodeProto {
            name: format!("{}/inlined", node.name),
            op_type: "Constant".into(),
            output: node.output.clone(),
            attribute: vec![AttributeProto { name: "value".into(), r#type: attribute_proto::AttributeType::Tensor as i32, t: Some(value), ..Default::default() }],
            ..Default::default()
        };
    }
    Ok(())
}

/// `Range(start, limit, -k)` with a constant negative step becomes
/// `start - Range(0, start - limit, k)`, which tract sizes correctly.
fn rewrite_negative_ranges(graph: &mut GraphProto) {
    let mut constants = std::collections::HashMap::new();
    for n in graph.node.iter().filter(|n| n.op_type == "Constant") {
        if let Some(t) = n.attribute.iter().find(|a| a.name == "value").and_then(|a| a.t.as_ref()) {
            constants.insert(n.output[0].clone(), t.clone());
        }
    }
    for t in &graph.initializer {
        constants.insert(t.name.clone(), t.clone());
    }
    let scalar = |t: &TensorProto| -> Option<(i32, f64)> {
        if !t.dims.iter().all(|&d| d == 1) {
            return None;
        }
        let v = match tensor_proto::DataType::try_from(t.data_type).ok()? {
            tensor_proto::DataType::Int64 => t.int64_data.first().map(|&v| v as f64).or_else(|| t.raw_data.get(..8).map(|b| i64::from_le_bytes(b.try_into().unwrap()) as f64))?,
            tensor_proto::DataType::Int32 => t.int32_data.first().map(|&v| v as f64).or_else(|| t.raw_data.get(..4).map(|b| i32::from_le_bytes(b.try_into().unwrap()) as f64))?,
            tensor_proto::DataType::Float => floats(t)?.first().map(|&v| v as f64)?,
            _ => return None,
        };
        Some((t.data_type, v))
    };
    let mut out = Vec::with_capacity(graph.node.len());
    for n in std::mem::take(&mut graph.node) {
        let step = (n.op_type == "Range" && n.input.len() == 3).then(|| constants.get(&n.input[2]).and_then(scalar)).flatten();
        let Some((dt, step)) = step.filter(|(_, s)| *s < 0.0) else {
            out.push(n);
            continue;
        };
        let p = &n.name;
        let constant = |name: String, v: f64| {
            let mut t = TensorProto { data_type: dt, ..Default::default() };
            match tensor_proto::DataType::try_from(dt) {
                Ok(tensor_proto::DataType::Int64) => t.int64_data = vec![v as i64],
                Ok(tensor_proto::DataType::Int32) => t.int32_data = vec![v as i32],
                _ => t.float_data = vec![v as f32],
            }
            NodeProto {
                op_type: "Constant".into(),
                output: vec![name],
                attribute: vec![AttributeProto { name: "value".into(), r#type: attribute_proto::AttributeType::Tensor as i32, t: Some(t), ..Default::default() }],
                ..Default::default()
            }
        };
        let op = |op: &str, inputs: Vec<String>, output: String| NodeProto { op_type: op.into(), input: inputs, output: vec![output], ..Default::default() };
        out.push(constant(format!("{p}/wugs_zero"), 0.0));
        out.push(constant(format!("{p}/wugs_step"), -step));
        out.push(op("Sub", vec![n.input[0].clone(), n.input[1].clone()], format!("{p}/wugs_count")));
        out.push(op("Range", vec![format!("{p}/wugs_zero"), format!("{p}/wugs_count"), format!("{p}/wugs_step")], format!("{p}/wugs_up")));
        out.push(op("Sub", vec![n.input[0].clone(), format!("{p}/wugs_up")], n.output[0].clone()));
    }
    graph.node = out;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_txt_reads_ids_and_marks_non_phones() {
        let v = Vocab::from_tokens_txt("<blk> 0\n<sos/eos> 1\n<unk> 2\n▁ 3\na 4\nʃ 5\n").unwrap();
        assert_eq!(v.blank, 0);
        assert_eq!(v.symbols[5], "ʃ");
        assert_eq!(v.skip, [1, 2, 3]);
        assert!(Vocab::from_tokens_txt("a 0\n").is_err());
        assert!(Vocab::from_tokens_txt("<blk> x\n").is_err());
    }
}
