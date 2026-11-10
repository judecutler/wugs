"""onnxruntime's log_probs for the real ZIPA INT8 release on the made-up
features `feats(T)` (the same formula as tests/zipformer.rs), for the
ignored `the_real_zipa_release_matches_onnxruntime` test.
Run: python3 -I real_zipa_reference.py <folder with model.int8.onnx>"""
import os, sys
import numpy as np, onnxruntime as ort
d = sys.argv[1]
sess = ort.InferenceSession(os.path.join(d, "model.int8.onnx"))
for T in (500, 3000):
    t = np.arange(T, dtype=np.float64)[:, None]
    f = np.arange(80, dtype=np.float64)[None, :]
    x = (np.sin(0.37 * t + 1.3 * f) * 2.0 + np.cos(0.011 * t * (f + 1.0))).astype(np.float32)[None]
    lp = sess.run(None, {"x": x, "x_lens": np.array([T], dtype=np.int64)})[0][0]
    lp.astype("<f4").tofile(os.path.join(d, f"ref.{T}.out.f32"))
    print(T, lp.shape)

