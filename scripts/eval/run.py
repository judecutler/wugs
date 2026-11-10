"""Transcribe every PRiSM set in <sets> with every model in <models>
(fetch_models.py's output) through wugs-asr-eval, the way wugs runs them.
Writes <out>/<model>.<set>.jsonl and the tool's speed/memory line to
<out>/<model>.<set>.log."""
import json, os, subprocess, sys

models_dir, sets_dir, out = sys.argv[1], sys.argv[2], sys.argv[3]
limit = sys.argv[sys.argv.index("--limit") + 1] if "--limit" in sys.argv else None
os.makedirs(out, exist_ok=True)
binary = os.path.join("target", "release", "wugs-asr-eval")
sets = []
for root, dirs, files in os.walk(sets_dir):
    if "wav.scp" in files and "text.good" in files:
        sets.append(root)
print("sets:", sets)
for m in json.load(open(os.path.join(models_dir, "models.json"))):
    d = os.path.join(models_dir, m["id"])
    p = m["params"]
    for s in sets:
        name = f"{m['id']}.{os.path.basename(s.rstrip('/'))}"
        cmd = [binary, "--runtime", m["runtime"], "--set", s, "--out", os.path.join(out, name + ".jsonl")]
        if m["runtime"] == "onnx-ctc-fbank":
            cmd += ["--model", os.path.join(d, p["onnx"]), "--vocab", os.path.join(d, p["tokens"])]
        else:
            cmd += ["--model", os.path.join(d, p["onnx"]), "--vocab", os.path.join(d, p["vocab"])]
            if p.get("normalize"):
                cmd.append("--normalize")
        if limit:
            cmd += ["--limit", limit]
        print(" ".join(cmd), flush=True)
        r = subprocess.run(cmd, capture_output=True, text=True)
        open(os.path.join(out, name + ".log"), "w").write(r.stderr)
        print(r.stderr[-2000:], flush=True)
        if r.returncode != 0:
            sys.exit(f"{name} failed")
