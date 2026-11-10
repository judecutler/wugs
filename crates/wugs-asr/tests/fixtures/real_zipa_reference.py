"""onnxruntime's log_probs for the real ZIPA INT8 release on the made-up
features `feats(T)` (the same formula as tests/zipformer.rs), for the
ignored `the_real_zipa_release_matches_onnxruntime` test.
Run: python3 -I real_zipa_reference.py <folder with model.int8.onnx>"""
import os, sys
import numpy as np, onnxruntime as ort
d = sys.argv[1]
sess = ort.InferenceSession(os.path.join(d, "model.int8.onnx"))
for T in (500, 3000, 517):
    t = np.arange(T, dtype=np.float64)[:, None]
    f = np.arange(80, dtype=np.float64)[None, :]
    x = (np.sin(0.37 * t + 1.3 * f) * 2.0 + np.cos(0.011 * t * (f + 1.0))).astype(np.float32)[None]
    lp = sess.run(None, {"x": x, "x_lens": np.array([T], dtype=np.int64)})[0][0]
    lp.astype("<f4").tofile(os.path.join(d, f"ref.{T}.out.f32"))
    print(T, lp.shape)

# How much does padding change onnxruntime's own transcript of the sample?
# (Diagnostic for the padded runs: the export's mask is sized by max(x_lens),
# so the padded graph uses the encoder input's length, as wugs does.)
sample = os.path.join(d, "sample.wav")
if os.path.exists(sample):
    import onnx, soundfile as sf, torch
    from onnx import helper, numpy_helper
    from lhotse.features.kaldi.extractors import Fbank, FbankConfig
    m = onnx.load(os.path.join(d, "model.int8.onnx")); g = m.graph
    tr = [n for n in g.node if n.op_type == "Transpose" and list(helper.get_attribute_value(n.attribute[0])) == [1, 0, 2]][0]
    nodes = []
    for n in g.node:
        if n.op_type == "ReduceMax" and not any(a.name == "axes" for a in n.attribute):
            nodes += [helper.make_node("Shape", [tr.input[0]], [n.name + "_s"]),
                      helper.make_node("Constant", [], [n.name + "_i"], value=numpy_helper.from_array(np.array(1, dtype=np.int64))),
                      helper.make_node("Gather", [n.name + "_s", n.name + "_i"], [n.output[0]], axis=0)]
        else:
            nodes.append(n)
    del g.node[:]; g.node.extend(nodes)
    padded_sess = ort.InferenceSession(m.SerializeToString())
    audio, _ = sf.read(sample)
    feats = Fbank(FbankConfig(num_filters=80, dither=0.0, snip_edges=False)).extract_batch([torch.from_numpy(audio).float()], sampling_rate=16000)[0].numpy()
    toks = {int(l.split()[1]): l.split()[0] for l in open(os.path.join(d, "tokens.txt"), encoding="utf-8") if l.strip()}
    def decode(lp):
        out, prev = [], -1
        for i in lp.argmax(-1):
            if i != prev and i != 0:
                out.append(toks[int(i)])
            prev = i
        return " ".join(out)
    T = feats.shape[0]
    plain = sess.run(None, {"x": feats[None], "x_lens": np.array([T], dtype=np.int64)})[0][0]
    for name, pad in (("last frame", None), ("lhotse LOG_EPSILON", np.log(1e-10)), ("zeros", 0.0)):
        for B in (600, 700, 1050):
            tail = np.repeat(feats[-1:], B - T, 0) if pad is None else np.full((B - T, 80), pad, np.float32)
            x = np.concatenate([feats, tail]).astype(np.float32)[None]
            lp, ll = padded_sess.run(None, {"x": x, "x_lens": np.array([T], dtype=np.int64)})
            lp = lp[0][: int(ll[0])]
            print(f"ORT padded to {B} with {name}: {decode(lp)}")
    print(f"ORT unpadded:     {decode(plain)}")
