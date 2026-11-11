//! The `onnx-ctc-fbank` runtime: CTC phone models that take 80-bin Kaldi
//! filterbank features, `x (1, T, 80)` and `x_lens (1,)`, and return
//! `log_probs (1, T', vocab)`. ZIPA's Zipformer CTC models are the first.
//!
//! tract cannot run icefall's Zipformer ONNX export as it comes, so the
//! graph is adjusted once at load (`tests/zipformer.rs` checks the result
//! against onnxruntime):
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
//! - the export computes shapes from runtime data (the padding mask, the
//!   attention's relative shift), which tract cannot type symbolically as
//!   they come; the rewrites in `symbolic_plan` free them, so one plan
//!   built at load, with the frame count left symbolic, serves every
//!   segment. A future export whose graph defeats a rewrite falls back to
//!   a plan per input length, with `x_lens` a constant; the last few are
//!   kept. Inputs are never padded: the export does not mask padding at
//!   batch one, and the real model's transcript of ZIPA's sample changed
//!   by a third with as little as 14 padded frames (any padding value),
//!   while unpadded runs reproduce ZIPA's own script exactly. Plans skip
//!   tract's optimizer: for the real model on a CI runner, an optimized
//!   plan took 8–11 s to build and 0.16–2 s to run (5 s and 30 s of
//!   audio), a decluttered one 2.7–8 s and 1–5 s.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tract_onnx::pb::{attribute_proto, tensor_proto, AttributeProto, GraphProto, ModelProto, NodeProto, TensorProto};
use tract_onnx::prelude::*;

use crate::fbank::{Fbank, BINS};
use crate::{ctc_greedy, model_err, Error, Recognition, Vocab, MAX_SECONDS, SAMPLE_RATE};

/// Plans kept at once: a batch run often meets the same length again.
const PLANS: usize = 4;

pub struct FbankRecognizer {
    /// The adjusted graph, parsed once; each plan starts from a clone.
    model: InferenceModel,
    vocab: Vocab,
    fbank: Fbank,
    plans: Mutex<VecDeque<(usize, Arc<TypedSimplePlan>)>>,
    /// One plan typed with the frame count left symbolic; when the
    /// rewrites below make the graph plannable that way, it replaces the
    /// per-length cache.
    symbolic: Option<Arc<TypedSimplePlan>>,
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
        // A future export whose graph defeats a rewrite falls back to the
        // per-length path below.
        let symbolic = symbolic_plan(&proto).ok();
        let model = tract_onnx::onnx().model_for_proto_model(&proto).map_err(model_err)?;
        Ok(FbankRecognizer { model, vocab, fbank: Fbank::new(), plans: Mutex::new(VecDeque::new()), symbolic })
    }

    /// Whether one symbolic-length plan serves every segment.
    pub fn has_symbolic_plan(&self) -> bool {
        self.symbolic.is_some()
    }

    /// Per-length plans built so far (the fallback path's cache).
    pub fn per_length_plans(&self) -> usize {
        self.plans.lock().expect("plans lock").len()
    }

    /// Drop the symbolic plan, for timing the fallback path in tests.
    #[doc(hidden)]
    pub fn drop_symbolic_plan(&mut self) {
        self.symbolic = None;
    }

    fn plan(&self, frames: usize) -> Result<Arc<TypedSimplePlan>, Error> {
        if let Some(plan) = &self.symbolic {
            return Ok(plan.clone());
        }
        {
            // Least recently used goes first when the cache is full.
            let mut plans = self.plans.lock().expect("plans lock");
            if let Some(i) = plans.iter().position(|(n, _)| *n == frames) {
                let hit = plans.remove(i).expect("found above");
                plans.push_front(hit.clone());
                return Ok(hit.1);
            }
        }
        let mut model = self.model.clone();
        model.set_input_fact(0, InferenceFact::dt_shape(DatumType::F32, tvec!(1.to_dim(), frames.to_dim(), BINS.to_dim()))).map_err(model_err)?;
        model.set_input_fact(1, InferenceFact::from(tract_ndarray::arr1(&[frames as i64]).into_tensor())).map_err(model_err)?;
        let x = model.node(model.input_outlets().map_err(model_err)?[0].node).name.clone();
        let model = model.with_input_names([x]).map_err(model_err)?.into_typed().map_err(model_err)?;
        let plan = model.into_decluttered().map_err(model_err)?.into_runnable().map_err(model_err)?;
        let mut plans = self.plans.lock().expect("plans lock");
        plans.retain(|(n, _)| *n != frames);
        plans.push_front((frames, plan.clone()));
        plans.truncate(PLANS);
        Ok(plan)
    }

    /// Log probabilities for filterbank features, as `(frames, vocab_len, flat)`.
    pub fn log_probs_for_features(&self, frames: usize, feats: Vec<f32>) -> Result<(usize, usize, Vec<f32>), Error> {
        if feats.len() != frames * BINS {
            return Err(Error::Input(format!("expected {frames} × {BINS} features")));
        }
        // The subsampling front needs a few frames to produce one.
        if frames < 16 {
            return Err(Error::Input("the segment is shorter than a tenth of a second".into()));
        }
        if frames > MAX_SECONDS as usize * 100 {
            return Err(Error::Input(format!("the segment is longer than {MAX_SECONDS} s")));
        }
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
        let flat = lp.to_plain_array_view::<f32>().map_err(model_err)?.iter().copied().collect();
        Ok((t, v, flat))
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
    let constants = constant_tensors(graph);
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
        // Named after the output, which is unique even when nodes are not named.
        let p = &n.output[0];
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

/// Constant tensors by output name: `Constant` nodes and initializers.
fn constant_tensors(graph: &GraphProto) -> std::collections::HashMap<String, TensorProto> {
    let mut constants = std::collections::HashMap::new();
    for n in graph.node.iter().filter(|n| n.op_type == "Constant") {
        for a in &n.attribute {
            let t = match a.name.as_str() {
                "value" => a.t.clone(),
                "value_int" => Some(TensorProto { data_type: tensor_proto::DataType::Int64 as i32, int64_data: vec![a.i], ..Default::default() }),
                "value_float" => Some(TensorProto { data_type: tensor_proto::DataType::Float as i32, float_data: vec![a.f], ..Default::default() }),
                _ => None,
            };
            if let Some(t) = t {
                constants.insert(n.output[0].clone(), t);
            }
        }
    }
    for t in &graph.initializer {
        constants.insert(t.name.clone(), t.clone());
    }
    constants
}

fn int_values(t: &TensorProto) -> Option<Vec<i64>> {
    match tensor_proto::DataType::try_from(t.data_type).ok()? {
        tensor_proto::DataType::Int64 if !t.int64_data.is_empty() => Some(t.int64_data.clone()),
        tensor_proto::DataType::Int64 if t.raw_data.len() % 8 == 0 => {
            Some(t.raw_data.chunks_exact(8).map(|c| i64::from_le_bytes(c.try_into().expect("8 bytes"))).collect())
        }
        tensor_proto::DataType::Int32 if !t.int32_data.is_empty() => Some(t.int32_data.iter().map(|&v| v as i64).collect()),
        tensor_proto::DataType::Int32 if t.raw_data.len() % 4 == 0 => {
            Some(t.raw_data.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().expect("4 bytes")) as i64).collect())
        }
        _ => None,
    }
}

fn int64_tensor(dims: Vec<i64>, vals: Vec<i64>) -> TensorProto {
    TensorProto { dims, data_type: tensor_proto::DataType::Int64 as i32, int64_data: vals, ..Default::default() }
}

fn const_node(name: &str, t: TensorProto) -> NodeProto {
    NodeProto {
        op_type: "Constant".into(),
        output: vec![name.into()],
        attribute: vec![AttributeProto { name: "value".into(), r#type: attribute_proto::AttributeType::Tensor as i32, t: Some(t), ..Default::default() }],
        ..Default::default()
    }
}

fn op_node(op: &str, inputs: &[&str], output: &str) -> NodeProto {
    NodeProto { op_type: op.into(), input: inputs.iter().map(|s| s.to_string()).collect(), output: vec![output.into()], ..Default::default() }
}

fn int_attr(name: &str, i: i64) -> AttributeProto {
    AttributeProto { name: name.into(), r#type: attribute_proto::AttributeType::Int as i32, i, ..Default::default() }
}

/// Drop the `x_lens` input and feed its consumers `Gather(Shape(x), [1])`,
/// the input's own frame count, so the mask length is a shape-derived
/// symbolic dim rather than runtime data. Exact because inputs are never
/// padded: `x_lens` always equals `x`'s second dim.
fn take_x_lens_from_shape(graph: &mut GraphProto) -> Result<(), Error> {
    if graph.input.len() != 2 {
        return Err(Error::Model(format!("expected inputs x and x_lens, got {}", graph.input.len())));
    }
    let x = graph.input[0].name.clone();
    let lens = graph.input.pop().expect("two inputs").name;
    for n in graph.node.iter_mut() {
        for i in n.input.iter_mut() {
            if *i == lens {
                *i = "wugs_x_lens".into();
            }
        }
    }
    let mut head = vec![
        op_node("Shape", &[&x], "wugs_x_shape"),
        const_node("wugs_x_t", int64_tensor(vec![1], vec![1])),
        op_node("Gather", &["wugs_x_shape", "wugs_x_t"], "wugs_x_lens"),
    ];
    head.append(&mut graph.node);
    graph.node = head;
    Ok(())
}

/// icefall builds its padding mask as `arange(frames) >= x_lens'` and
/// feeds it only to `Where` conditions (attention and convolution
/// masked_fill). With `x_lens` taken from the input's own shape the
/// mask is identically false (both the arange's limit and the compared
/// length derive from the one real frame count), so each such `Where`
/// passes its data input through. tract cannot type the mask's own
/// arithmetic symbolically (the arange's `Expand` has a data-dependent
/// shape), so the mask is removed rather than followed: the `Where`s
/// become `Identity(data)` and the dead mask subgraph is pruned. The
/// pattern is verified before anything changes; a comparison that is
/// not this mask, or a mask that reaches anything but a `Where`
/// condition, leaves the graph as it was.
fn drop_padding_masks(graph: &mut GraphProto) -> Result<(), Error> {
    use std::collections::{HashMap, HashSet};
    let constants = constant_tensors(graph);
    let by_output: HashMap<&str, usize> = graph.node.iter().enumerate().flat_map(|(i, n)| n.output.iter().map(move |o| (o.as_str(), i))).collect();

    // From `name` up to `wugs_x_lens` through length arithmetic only;
    // every other non-constant leaf or op disqualifies the trace.
    let reaches_x_lens = |name: &str| -> bool {
        let mut work = vec![name.to_string()];
        let mut seen = HashSet::new();
        let mut found = false;
        while let Some(t) = work.pop() {
            if !seen.insert(t.clone()) || constants.contains_key(&t) {
                continue;
            }
            if t == "wugs_x_lens" {
                found = true;
                continue;
            }
            let Some(&i) = by_output.get(t.as_str()) else { return false };
            let n = &graph.node[i];
            match n.op_type.as_str() {
                "Unsqueeze" | "Squeeze" | "Cast" | "Div" | "Sub" | "Add" | "Mul" | "Min" | "Max" | "ReduceMax" | "Reshape" | "Gather" | "Concat" | "Shape" => {
                    work.extend(n.input.iter().cloned())
                }
                _ => return false,
            }
        }
        found
    };

    // An arange over frames: input[0] chains through broadcast ops to a
    // `Range(0, length, 1)` whose length is x_lens-derived.
    let is_frame_arange = |name: &str| -> bool {
        let mut t = name.to_string();
        for _ in 0..8 {
            let Some(&i) = by_output.get(t.as_str()) else { return false };
            let n = &graph.node[i];
            match n.op_type.as_str() {
                "Expand" | "Unsqueeze" | "Cast" => t = n.input[0].clone(),
                "Range" => {
                    let scalar = |input: &String| constants.get(input).and_then(int_values).filter(|v| v.len() == 1).map(|v| v[0]);
                    return scalar(&n.input[0]) == Some(0) && scalar(&n.input[2]) == Some(1) && reaches_x_lens(&n.input[1]);
                }
                _ => return false,
            }
        }
        false
    };

    let masks: Vec<String> = graph
        .node
        .iter()
        .filter(|n| n.op_type == "GreaterOrEqual" && is_frame_arange(&n.input[0]) && reaches_x_lens(&n.input[1]))
        .map(|n| n.output[0].clone())
        .collect();
    if masks.is_empty() {
        return Err(Error::Model("no padding mask found".into()));
    }

    let mut consumers: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
    for (i, n) in graph.node.iter().enumerate() {
        for (a, input) in n.input.iter().enumerate() {
            consumers.entry(input).or_default().push((i, a));
        }
    }
    let mut wheres = Vec::new();
    let mut work = masks;
    let mut seen = HashSet::new();
    while let Some(t) = work.pop() {
        if !seen.insert(t.clone()) {
            continue;
        }
        for &(i, a) in consumers.get(t.as_str()).map_or(&[][..], |v| v) {
            let n = &graph.node[i];
            match n.op_type.as_str() {
                "Where" if a == 0 => wheres.push(i),
                "Slice" | "Unsqueeze" | "Expand" | "Cast" | "Squeeze" | "Reshape" | "Transpose" | "Gather" if a == 0 => work.extend(n.output.iter().cloned()),
                op => return Err(Error::Model(format!("the padding mask reaches {op} {} (as input {a}), not a Where condition", n.name))),
            }
        }
    }
    for i in wheres {
        let n = &graph.node[i];
        graph.node[i] = op_node("Identity", &[&n.input[2].clone()], &n.output[0].clone());
    }

    // The mask chain is now dead; nodes ONNX lists in topological order,
    // so one reverse pass finds everything the outputs still need.
    let mut needed: HashSet<String> = graph.output.iter().map(|o| o.name.clone()).collect();
    let mut keep = vec![false; graph.node.len()];
    for (i, n) in graph.node.iter().enumerate().rev() {
        if n.output.iter().any(|o| needed.contains(o)) {
            keep[i] = true;
            needed.extend(n.input.iter().cloned());
        }
    }
    let mut keep = keep.into_iter();
    graph.node.retain(|_| keep.next().expect("one flag per node"));
    Ok(())
}

/// A strided `Slice` (constant step > 1) becomes
/// `Gather(data, Range(start, Min(end, dim), step), axis)`: tract sizes
/// the Range symbolically where it cannot slice with a stride.
fn rewrite_strided_slices(graph: &mut GraphProto) {
    let constants = constant_tensors(graph);
    let ints = |name: &String| constants.get(name).and_then(int_values);
    let mut out = Vec::with_capacity(graph.node.len());
    for n in std::mem::take(&mut graph.node) {
        let parts = (n.op_type == "Slice" && n.input.len() == 5)
            .then(|| match (ints(&n.input[1]).as_deref(), ints(&n.input[2]).as_deref(), ints(&n.input[3]).as_deref(), ints(&n.input[4]).as_deref()) {
                (Some(&[start]), Some(&[end]), Some(&[axis]), Some(&[step])) if step > 1 && start >= 0 && end >= 0 && axis >= 0 => Some((start, end, axis, step)),
                _ => None,
            })
            .flatten();
        let Some((start, end, axis, step)) = parts else {
            out.push(n);
            continue;
        };
        let p = n.output[0].clone();
        out.push(op_node("Shape", &[&n.input[0]], &format!("{p}/wugs_shape")));
        out.push(const_node(&format!("{p}/wugs_axis"), int64_tensor(vec![1], vec![axis])));
        out.push(op_node("Gather", &[&format!("{p}/wugs_shape"), &format!("{p}/wugs_axis")], &format!("{p}/wugs_dim")));
        out.push(const_node(&format!("{p}/wugs_end"), int64_tensor(vec![1], vec![end])));
        out.push(op_node("Min", &[&format!("{p}/wugs_end"), &format!("{p}/wugs_dim")], &format!("{p}/wugs_limit")));
        out.push(op_node("Squeeze", &[&format!("{p}/wugs_limit")], &format!("{p}/wugs_limit_s")));
        out.push(const_node(&format!("{p}/wugs_start"), int64_tensor(vec![], vec![start])));
        out.push(const_node(&format!("{p}/wugs_step"), int64_tensor(vec![], vec![step])));
        out.push(op_node("Range", &[&format!("{p}/wugs_start"), &format!("{p}/wugs_limit_s"), &format!("{p}/wugs_step")], &format!("{p}/wugs_idx")));
        let mut gather = op_node("Gather", &[&n.input[0], &format!("{p}/wugs_idx")], &p);
        gather.attribute.push(int_attr("axis", axis));
        out.push(gather);
    }
    graph.node = out;
}

/// torch lowers `expand` to `Expand(data, Where(Equal(shape, -1),
/// ones, shape))`, and tract cannot give the Where's output a fixed
/// length. No element of these shapes is ever -1 (they are gathered
/// dims and positive constants), so the Where returns `shape` itself:
/// feed Expand the shape directly.
fn rewrite_expand_where(graph: &mut GraphProto) {
    let constants = constant_tensors(graph);
    let by_output: std::collections::HashMap<String, usize> =
        graph.node.iter().enumerate().flat_map(|(i, n)| n.output.first().cloned().map(|o| (o, i))).collect();
    let mut edits = Vec::new();
    for (i, n) in graph.node.iter().enumerate().filter(|(_, n)| n.op_type == "Expand") {
        let Some(w) = by_output.get(&n.input[1]).map(|&w| &graph.node[w]).filter(|w| w.op_type == "Where") else { continue };
        let mut shape = w.input[2].clone();
        // The shape also passes a no-op `Reshape(shape, [-1])` whose
        // output length tract loses; step through to the Concat.
        while let Some(r) = by_output.get(&shape).map(|&r| &graph.node[r]) {
            if r.op_type == "Reshape" && constants.get(&r.input[1]).and_then(int_values).as_deref() == Some(&[-1]) {
                shape = r.input[0].clone();
            } else {
                break;
            }
        }
        edits.push((i, shape));
    }
    for (i, shape) in edits {
        graph.node[i].input[1] = shape;
    }
}

/// The slice that cuts the current window out of an inlined positional
/// table has data-dependent bounds, and tract's symbolic length for it
/// comes out wrong (clamped against the table). `Gather(table,
/// Range(start, end, 1))` has the exact symbolic length `end - start`.
/// Only slices whose data is one of our own `/inlined` tables qualify,
/// so the bounds are known to lie inside the table and Slice's clamping
/// is not missed.
fn rewrite_positional_slices(graph: &mut GraphProto) {
    let constants = constant_tensors(graph);
    let inlined: std::collections::HashSet<String> = graph
        .node
        .iter()
        .filter(|n| n.op_type == "Constant" && n.name.ends_with("/inlined"))
        .map(|n| n.output[0].clone())
        .collect();
    let mut out = Vec::with_capacity(graph.node.len());
    for n in std::mem::take(&mut graph.node) {
        let fits = n.op_type == "Slice"
            && n.input.len() >= 3
            && inlined.contains(&n.input[0])
            && n.input.get(3).map_or(true, |a| constants.get(a).and_then(int_values).as_deref() == Some(&[0]))
            && n.input.get(4).map_or(true, |s| constants.get(s).and_then(int_values).as_deref() == Some(&[1]));
        if !fits {
            out.push(n);
            continue;
        }
        let p = n.output[0].clone();
        out.push(op_node("Squeeze", &[&n.input[1]], &format!("{p}/wugs_start")));
        out.push(op_node("Squeeze", &[&n.input[2]], &format!("{p}/wugs_end")));
        out.push(const_node(&format!("{p}/wugs_one"), int64_tensor(vec![], vec![1])));
        out.push(op_node("Range", &[&format!("{p}/wugs_start"), &format!("{p}/wugs_end"), &format!("{p}/wugs_one")], &format!("{p}/wugs_idx")));
        out.push(op_node("Gather", &[&n.input[0], &format!("{p}/wugs_idx")], &p));
    }
    graph.node = out;
}

/// The data tensor's rank, as far as the proto says: a `Transpose`
/// fixes it with its perm, a constant with its dims, and the ops the
/// attention chains them through keep it.
fn rank_of(graph: &GraphProto, constants: &std::collections::HashMap<String, TensorProto>, name: &str, depth: usize) -> Option<usize> {
    if depth > 24 {
        return None;
    }
    if let Some(t) = constants.get(name) {
        return Some(t.dims.len());
    }
    let n = graph.node.iter().find(|n| n.output.iter().any(|o| o == name))?;
    match n.op_type.as_str() {
        "Transpose" => n.attribute.iter().find(|a| a.name == "perm").map(|a| a.ints.len()),
        "MatMul" => {
            let a = rank_of(graph, constants, &n.input[0], depth + 1);
            let b = rank_of(graph, constants, &n.input[1], depth + 1);
            Some(a?.max(b?))
        }
        "Add" | "Sub" | "Mul" | "Div" | "Softmax" | "Cast" | "Identity" | "Relu" | "Where" | "Slice" | "DequantizeLinear" => {
            rank_of(graph, constants, &n.input[0], depth + 1)
        }
        _ => None,
    }
}

/// The attention's relative shift ends in `x.reshape(-1, x.size(-1))`,
/// whose -1 tract must divide out of a symbolic product it has already
/// expanded (`2k(2k-1)` becomes `4k²-2k`, and `2k-1` no longer divides
/// it). Written as the full target shape, `(d0·…·d_{r-2}, d_{r-1})`,
/// no division happens: the products are built with `Mul`, which tract
/// folds over symbolic dims, and the Reshape then matches group
/// volumes by equality. (tract's own `Flatten` refuses two symbolic
/// dims anywhere, so the shape is spelled out instead.) Only a
/// `Reshape` to `(-1, d)` where `d` is gathered from the data's own
/// last axis qualifies, and the data's rank must be established from
/// the proto; any other is left for tract.
fn rewrite_flatten_reshapes(graph: &mut GraphProto) {
    use std::collections::HashMap;
    let constants = constant_tensors(graph);
    let by_output: HashMap<&str, usize> = graph.node.iter().enumerate().flat_map(|(i, n)| n.output.iter().map(move |o| (o.as_str(), i))).collect();
    let mut flattens: Vec<(usize, usize)> = Vec::new();
    for (i, n) in graph.node.iter().enumerate() {
        if n.op_type != "Reshape" {
            continue;
        }
        let Some(&c) = by_output.get(n.input[1].as_str()) else { continue };
        let c = &graph.node[c];
        if c.op_type != "Concat" || c.input.len() != 2 || constants.get(&c.input[0]).and_then(int_values).as_deref() != Some(&[-1]) {
            continue;
        }
        // The tail must be the data's own last dim: Unsqueeze(Gather(
        // Shape(data), rank-1)).
        let Some(u) = by_output.get(c.input[1].as_str()).map(|&u| &graph.node[u]).filter(|u| u.op_type == "Unsqueeze") else { continue };
        let Some(gather) = by_output.get(u.input[0].as_str()).map(|&g| &graph.node[g]).filter(|g| g.op_type == "Gather") else { continue };
        let Some(shape) = by_output.get(gather.input[0].as_str()).map(|&s| &graph.node[s]).filter(|s| s.op_type == "Shape") else { continue };
        if shape.input[0] != n.input[0] {
            continue;
        }
        let Some(&[axis]) = constants.get(&gather.input[1]).and_then(int_values).as_deref() else { continue };
        if rank_of(graph, &constants, &n.input[0], 0) == Some(axis as usize + 1) {
            flattens.push((i, axis as usize));
        }
    }
    let flattens: HashMap<usize, usize> = flattens.into_iter().collect();
    let mut out = Vec::with_capacity(graph.node.len());
    for (i, mut n) in std::mem::take(&mut graph.node).into_iter().enumerate() {
        if let Some(&last) = flattens.get(&i) {
            let p = n.output[0].clone();
            out.push(op_node("Shape", &[&n.input[0]], &format!("{p}/wugs_shape")));
            let dim = |d: usize, name: String, out: &mut Vec<NodeProto>| {
                out.push(const_node(&format!("{name}_i"), int64_tensor(vec![1], vec![d as i64])));
                out.push(op_node("Gather", &[&format!("{p}/wugs_shape"), &format!("{name}_i")], &name));
            };
            dim(0, format!("{p}/wugs_d0"), &mut out);
            let mut prod = format!("{p}/wugs_d0");
            for d in 1..last {
                dim(d, format!("{p}/wugs_d{d}"), &mut out);
                out.push(op_node("Mul", &[&prod, &format!("{p}/wugs_d{d}")], &format!("{p}/wugs_prod{d}")));
                prod = format!("{p}/wugs_prod{d}");
            }
            dim(last, format!("{p}/wugs_last"), &mut out);
            out.push(NodeProto {
                op_type: "Concat".into(),
                input: vec![prod, format!("{p}/wugs_last")],
                output: vec![format!("{p}/wugs_newshape")],
                attribute: vec![int_attr("axis", 0)],
                ..Default::default()
            });
            n.input[1] = format!("{p}/wugs_newshape");
        }
        out.push(n);
    }
    graph.node = out;
}

/// The relative shift's gather indices are built from the symbolic
/// length, so in the typed model they are dim-expression tensors, which
/// `GatherElements` cannot evaluate (it casts without the run's symbol
/// values). An ONNX-level cast is elided at typing, where everything is
/// still i64, so the cast goes into the typed model: tract's own `Cast`
/// resolves dim expressions against the run's symbols.
fn concretize_gather_indices(model: &mut TypedModel) -> Result<(), Error> {
    use tract_onnx::tract_core::internal::InletId;
    use tract_onnx::tract_core::ops::array::GatherElements;
    let gathers: Vec<(usize, OutletId, String)> = model
        .nodes()
        .iter()
        .filter(|n| n.op_is::<GatherElements>())
        .map(|n| (n.id, n.inputs[1], n.name.clone()))
        .collect();
    for (id, outlet, name) in gathers {
        if model.outlet_fact(outlet).map_err(model_err)?.datum_type != DatumType::TDim {
            continue;
        }
        let wire = model.wire_node(format!("{name}.wugs_i64"), tract_onnx::tract_core::ops::cast::cast(DatumType::I64), &[outlet]).map_err(model_err)?[0];
        model.add_edge(wire, InletId::new(id, 1)).map_err(model_err)?;
    }
    Ok(())
}

/// One decluttered plan with the frame count left symbolic, after the
/// rewrites that free every shape of runtime data and of -1 divisions.
fn symbolic_plan(proto: &ModelProto) -> Result<Arc<TypedSimplePlan>, Error> {
    let mut proto = proto.clone();
    let graph = proto.graph.as_mut().ok_or_else(|| Error::Model("the ONNX file has no graph".into()))?;
    take_x_lens_from_shape(graph)?;
    drop_padding_masks(graph)?;
    rewrite_strided_slices(graph);
    rewrite_positional_slices(graph);
    rewrite_expand_where(graph);
    rewrite_flatten_reshapes(graph);
    let mut model = tract_onnx::onnx().model_for_proto_model(&proto).map_err(model_err)?;
    let t = model.sym("T");
    model.set_input_fact(0, InferenceFact::dt_shape(DatumType::F32, tvec!(1.to_dim(), TDim::Sym(t), BINS.to_dim()))).map_err(model_err)?;
    let mut model = model.into_typed().map_err(model_err)?;
    concretize_gather_indices(&mut model)?;
    model.into_decluttered().map_err(model_err)?.into_runnable().map_err(model_err)
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
