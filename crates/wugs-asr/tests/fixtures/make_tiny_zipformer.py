"""Fixtures for the `onnx-ctc-fbank` runtime (ZIPA-style Zipformer CTC models).

Run: python3 -I make_tiny_zipformer.py <zipa checkout> <this folder>
with torch, onnx, onnxruntime and lhotse installed. <zipa checkout> is
https://github.com/lingjzhu/zipa (MIT); its icefall-derived model code is
imported, with two small stand-ins for the `k2` and `icefall` packages
written to a temporary folder (only the inference paths are used).

Writes:
- tiny-zipformer.onnx: a randomly initialised Zipformer2 CTC model with
  ZIPA-CR-small's topology (six stacks, downsampling 1,2,4,8,4,2, the
  Conv2dSubsampling front, a LogSoftmax head over 127 tokens) and tiny
  widths, exported the way ZIPA's export-onnx-ctc.py does (jit trace,
  opset 13, inputs x and x_lens);
- tiny-zipformer.<T>.out.f32: onnxruntime's log_probs for T frames of the
  made-up features `feats(T)` below (the Rust test computes the same
  formula; for T=3000, 30 s, only the first and last 40 output frames are
  kept, to keep the file small);
- tiny-zipformer.int8.onnx and tiny-zipformer.int8.173.out.f32: the same
  model quantized as ZIPA quantizes its INT8 release (quantize_dynamic,
  MatMul only, QInt8 weights) and onnxruntime's log_probs for 173 frames;
- fbank.audio.f32 / fbank.lhotse.f32: a synthetic 1.23 s signal and the
  80-bin features lhotse computes for it with ZIPA's settings
  (FbankConfig(num_filters=80, dither=0.0, snip_edges=False)).
All files are little-endian float32.
"""
import os, sys, tempfile, textwrap
zipa, out = sys.argv[1], sys.argv[2]
stubs = tempfile.mkdtemp()
os.makedirs(os.path.join(stubs, "icefall"))
open(os.path.join(stubs, "k2.py"), "w").write(textwrap.dedent("""
    import torch
    def swoosh_l_forward(x):
        return torch.logaddexp(torch.zeros_like(x), x - 4.0) - 0.08 * x - 0.035
    def swoosh_r_forward(x):
        return torch.logaddexp(torch.zeros_like(x), x - 1.0) - 0.08 * x - 0.313261687
    swoosh_l = swoosh_l_forward
    swoosh_r = swoosh_r_forward
    class RaggedTensor: pass
"""))
open(os.path.join(stubs, "icefall", "__init__.py"), "w").write("")
open(os.path.join(stubs, "icefall", "utils.py"), "w").write(textwrap.dedent("""
    import torch
    def make_pad_mask(lengths, max_len=0):
        max_len = max(max_len, lengths.max())
        n = lengths.size(0)
        seq_range = torch.arange(0, max_len, device=lengths.device)
        return seq_range.unsqueeze(0).expand(n, max_len) >= lengths.unsqueeze(-1)
"""))
sys.path.insert(0, stubs)
sys.path.insert(0, os.path.join(zipa, "zipformer_crctc"))
import numpy as np, torch, torch.nn as nn
from zipformer import Zipformer2
from subsampling import Conv2dSubsampling
from scaling_converter import convert_scaled_to_non_scaled
from icefall.utils import make_pad_mask

torch.manual_seed(7)
dims = (16, 16, 24, 32, 24, 16)
enc = Zipformer2(
    output_downsampling_factor=1, downsampling_factor=(1, 2, 4, 8, 4, 2),
    num_encoder_layers=(1, 1, 1, 1, 1, 1), encoder_dim=dims, encoder_unmasked_dim=(16,) * 6,
    query_head_dim=(8,), pos_head_dim=(4,), value_head_dim=(4,), pos_dim=16, num_heads=(2,) * 6,
    feedforward_dim=(32, 32, 48, 64, 48, 32), cnn_module_kernel=(31, 31, 15, 15, 15, 31),
    causal=False, chunk_size=(-1,), left_context_frames=(-1,),
)
embed = Conv2dSubsampling(in_channels=80, out_channels=dims[0])
ctc = nn.Sequential(nn.Dropout(p=0.1), nn.Linear(max(dims), 127), nn.LogSoftmax(dim=-1))

class OnnxModel(nn.Module):
    def __init__(s, e, emb, c):
        super().__init__(); s.encoder = e; s.encoder_embed = emb; s.ctc_output = c
    def forward(s, x, x_lens):
        x, x_lens = s.encoder_embed(x, x_lens)
        mask = make_pad_mask(x_lens)
        o, l = s.encoder(x.permute(1, 0, 2), x_lens, mask)
        return s.ctc_output(o.permute(1, 0, 2)), l

m = OnnxModel(enc, embed, ctc).eval()
convert_scaled_to_non_scaled(m, inplace=True, is_onnx=True)
x, xl = torch.zeros(1, 100, 80), torch.tensor([100], dtype=torch.int64)
traced = torch.jit.trace(m, (x, xl), check_trace=False)
path = os.path.join(out, "tiny-zipformer.onnx")
torch.onnx.export(traced, (x, xl), path, opset_version=13, input_names=["x", "x_lens"],
    output_names=["log_probs", "log_probs_len"],
    dynamic_axes={"x": {0: "N", 1: "T"}, "x_lens": {0: "N"}, "log_probs": {0: "N", 1: "T"}, "log_probs_len": {0: "N"}},
    dynamo=False)

import onnxruntime as ort
sess = ort.InferenceSession(path)
def feats_for(T):
    t = np.arange(T, dtype=np.float64)[:, None]
    f = np.arange(80, dtype=np.float64)[None, :]
    return (np.sin(0.37 * t + 1.3 * f) * 2.0 + np.cos(0.011 * t * (f + 1.0))).astype(np.float32)[None]

for T in (173, 3000):
    feats = feats_for(T)
    lp = sess.run(None, {"x": feats, "x_lens": np.array([T], dtype=np.int64)})[0][0]
    if T == 3000:
        lp = np.concatenate([lp[:40], lp[-40:]])
    lp.astype("<f4").tofile(os.path.join(out, f"tiny-zipformer.{T}.out.f32"))

from onnxruntime.quantization import QuantType, quantize_dynamic
q = os.path.join(out, "tiny-zipformer.int8.onnx")
quantize_dynamic(model_input=path, model_output=q, op_types_to_quantize=["MatMul"], weight_type=QuantType.QInt8)
lp = ort.InferenceSession(q).run(None, {"x": feats_for(173), "x_lens": np.array([173], dtype=np.int64)})[0][0]
lp.astype("<f4").tofile(os.path.join(out, "tiny-zipformer.int8.173.out.f32"))

from lhotse.features.kaldi.extractors import Fbank, FbankConfig
n = int(1.23 * 16000)
t = np.arange(n) / 16000
audio = (0.3 * np.sin(2 * np.pi * 220 * t) * np.sin(2 * np.pi * 3 * t)
         + 0.05 * np.sin(2 * np.pi * 1750 * t)
         + 0.02 * np.random.default_rng(5).standard_normal(n)).astype(np.float32)
f = Fbank(FbankConfig(num_filters=80, dither=0.0, snip_edges=False))
feats = f.extract_batch([torch.from_numpy(audio)], sampling_rate=16000)[0].numpy()
audio.astype("<f4").tofile(os.path.join(out, "fbank.audio.f32"))
feats.astype("<f4").tofile(os.path.join(out, "fbank.lhotse.f32"))
print("frames", feats.shape, "onnx bytes", os.path.getsize(path))
