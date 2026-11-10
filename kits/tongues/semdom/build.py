"""Build kits/tongues/semdom/semdom-v4-en.jsonl from a sillsdev/semdom checkout.

Usage: python3 build.py <path to sillsdev/semdom checkout> <output .jsonl>
Needs PyYAML. Output: one JSON object per domain, in outline order.
"""
import json, sys, pathlib, yaml
src, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
rows = []
for p in (src / "data/v4/domains").rglob("*.yaml"):
    d = yaml.safe_load(p.read_text(encoding="utf-8"))
    code = str(d["code"])
    qs = [[q.get("q", "").strip(), " ".join(str(q.get("words", "") or "").split())] for q in (d.get("questions") or [])]
    rows.append({"code": code, "guid": d["guid"].upper(), "name": " ".join(d["name"].split()),
                 "description": " ".join(str(d.get("description", "") or "").split()), "questions": qs})
rows.sort(key=lambda r: [int(x) for x in r["code"].split(".")])
codes = {r["code"] for r in rows}
for r in rows:
    parent = r["code"].rsplit(".", 1)[0] if "." in r["code"] else None
    assert parent is None or parent in codes, r["code"]
assert len({r["guid"] for r in rows}) == len(rows)
with out.open("w", encoding="utf-8", newline="\n") as f:
    for r in rows:
        f.write(json.dumps(r, ensure_ascii=False, separators=(",", ":")) + "\n")
print(len(rows), out.stat().st_size)
