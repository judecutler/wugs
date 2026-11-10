"""Score every <out>/*.jsonl with PRiSM's own phone recognition scorer
(src/metrics/phone_recognition.py at the pinned commit), then print a
markdown table of PFER and PER per model and set with the speed and memory
wugs-asr-eval logged, and write <out>/scores.json for the manifests'
[evaluation] tables."""
import csv, glob, json, os, re, subprocess, sys

prism, out = sys.argv[1], sys.argv[2]
rows, scores = [], {}
for f in sorted(glob.glob(os.path.join(out, "*.jsonl"))):
    name = os.path.basename(f)[: -len(".jsonl")]
    model, dataset = name.split(".", 1)
    csv_path = os.path.abspath(os.path.join(out, name + ".csv"))
    if os.path.exists(csv_path):
        os.remove(csv_path)
    r = subprocess.run(
        [sys.executable, "-m", "src.metrics.phone_recognition", "--prediction_file", os.path.abspath(f),
         "--evaluation_name", name, "--gt_field", "target", "--pred_field", "processed_transcript",
         "--key_field", "utt_id", "--output_file", csv_path],
        cwd=prism, capture_output=True, text=True, env={**os.environ, "PYTHONPATH": prism},
    )
    if r.returncode != 0:
        print(f"## {name}: scorer failed\n```\n{r.stderr[-3000:]}\n```", file=sys.stderr)
        continue
    result = next(csv.DictReader(open(csv_path)))
    log = os.path.join(out, name + ".log")
    speed = open(log).read() if os.path.exists(log) else ""
    rtf = re.search(r"real-time factor ([0-9.]+)", speed)
    mem = re.search(r"peak memory (\S+) MB", speed)
    rows.append((model, dataset, result["N"], result["PFER"], result["PER (%)"], rtf.group(1) if rtf else "?", mem.group(1) if mem else "?"))
    scores.setdefault(model, {})[dataset] = {"N": int(result["N"]), "PFER": float(result["PFER"]), "PER": float(result["PER (%)"])}
print("| model | set | utterances | PFER (%) | PER (%) | real-time factor | peak memory (MB) |")
print("| --- | --- | --- | --- | --- | --- | --- |")
for row in rows:
    print("| " + " | ".join(str(c) for c in row) + " |")
json.dump(scores, open(os.path.join(out, "scores.json"), "w"), indent=1)
