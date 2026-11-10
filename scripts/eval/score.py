"""Score every <out>/*.jsonl with PRiSM's own phone recognition scorer
(src/metrics/phone_recognition.py at the pinned commit) and print a
markdown table of PFER and PER per model and set, with the speed and
memory lines wugs-asr-eval logged."""
import glob, os, subprocess, sys

prism, out = sys.argv[1], sys.argv[2]
rows = []
for f in sorted(glob.glob(os.path.join(out, "*.jsonl"))):
    name = os.path.basename(f)[: -len(".jsonl")]
    r = subprocess.run(
        [sys.executable, "-m", "src.metrics.phone_recognition", "--prediction_file", os.path.abspath(f),
         "--gt_field", "target", "--pred_field", "processed_transcript", "--key_field", "utt_id",
         "--output_file", os.path.abspath(os.path.join(out, name + ".csv"))],
        cwd=prism, capture_output=True, text=True, env={**os.environ, "PYTHONPATH": prism},
    )
    print(f"## {name}\n```\n{r.stdout[-4000:]}\n{r.stderr[-2000:]}\n```", file=sys.stderr)
    log = os.path.join(out, name + ".log")
    speed = (open(log).read().strip().splitlines()[-1:] if os.path.exists(log) else []) or [""]
    rows.append((name, r.returncode, speed[0]))
print("| model.set | scorer exit | speed and memory |\n| --- | --- | --- |")
for name, code, speed in rows:
    print(f"| {name} | {code} | {speed} |")
