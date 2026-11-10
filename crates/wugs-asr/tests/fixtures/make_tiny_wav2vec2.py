"""Build the tiny wav2vec2 phone-model fixtures for tests/tract.rs.

The architecture is Hugging Face's Wav2Vec2ForCTC as the large phone
models use it (layer-norm feature extractor, stable layer norm, grouped
positional convolution), shrunk and randomly initialized, then exported
with torch.onnx exactly as published wav2vec2 ONNX models are: opset 17,
a dynamic number of samples. tract must load and run this graph, and its
logits must match PyTorch's, saved alongside.

Run from this directory: python make_tiny_wav2vec2.py
(needs torch, transformers, numpy; the outputs are committed, so CI needs none of them).
"""
import json

import numpy as np
import torch
from transformers import Wav2Vec2Config, Wav2Vec2ForCTC

torch.manual_seed(7)
vocab = ["<pad>", "<s>", "</s>", "<unk>", "a", "b", "k", "o", "p", "ʃ", "ə", "ː"]
config = Wav2Vec2Config(
    vocab_size=len(vocab),
    hidden_size=32,
    num_hidden_layers=2,
    num_attention_heads=2,
    intermediate_size=64,
    conv_dim=(16, 16, 16, 16, 16, 16, 16),
    feat_extract_norm="layer",
    do_stable_layer_norm=True,
    num_conv_pos_embeddings=16,
    num_conv_pos_embedding_groups=4,
    pad_token_id=0,
    ctc_loss_reduction="mean",
)
model = Wav2Vec2ForCTC(config).eval()

audio = np.sin(np.arange(16000 * 1.3) / 16000 * 2 * np.pi * 220).astype(np.float32) * 0.3
audio += np.random.default_rng(1).normal(0, 0.05, audio.shape).astype(np.float32)
x = (audio - audio.mean()) / np.sqrt(audio.var() + 1e-7)
with torch.no_grad():
    logits = model(torch.from_numpy(x)[None]).logits[0].numpy()

def export(m, inp, name):
    torch.onnx.export(
        m, (inp,), name, input_names=["input_values"], output_names=["logits"],
        dynamic_axes={"input_values": {1: "samples"}, "logits": {1: "frames"}},
        opset_version=17, dynamo=False,
    )

# fp32 first: .half() converts the model in place.
export(model, torch.zeros(1, 16000), "tiny-wav2vec2.onnx")
export(model.half(), torch.zeros(1, 16000).half(), "tiny-wav2vec2.fp16.onnx")

with open("tiny-wav2vec2.vocab.json", "w", encoding="utf-8") as f:
    json.dump({s: i for i, s in enumerate(vocab)}, f, ensure_ascii=False)
audio.astype("<f4").tofile("tiny-wav2vec2.audio.f32")
np.savetxt("tiny-wav2vec2.logits.txt", logits, fmt="%.9g")
print("frames", logits.shape)

# A copy whose input is declared `(batch, length)` and whose intermediate
# shapes are recorded with those symbols, as Hugging Face Optimum exports
# are (the catalogued wav2vec2 model is one). Loading must ignore them.
import onnx
m = onnx.load("tiny-wav2vec2.onnx")
dims = m.graph.input[0].type.tensor_type.shape.dim
dims[0].ClearField("dim_value"); dims[0].dim_param = "batch"
dims[1].dim_param = "length"
m = onnx.shape_inference.infer_shapes(m)
onnx.save(m, "tiny-wav2vec2.symbolic.onnx")
