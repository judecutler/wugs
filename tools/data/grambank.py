"""Build datasets/grambank/ from a clone of https://github.com/grambank/grambank.

    git clone --depth 1 https://github.com/grambank/grambank /tmp/gb
    python3 tools/data/grambank.py /tmp/gb datasets/grambank

features.tsv: one row per feature in Grambank's order: id, question, codes
("0=absent|1=present"). values.tsv: one row per language: its Glottocode and
one character per feature in that order (the coded value, `?` where the
coders could not tell, `.` where it is not coded). Comments, sources and
coders stay in Grambank itself; the app links each language to its page.
"""
import csv
import sys
from pathlib import Path

src, out = Path(sys.argv[1]) / "cldf", Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
features = list(csv.DictReader(open(src / "parameters.csv", encoding="utf-8")))
codes = {}
for c in csv.DictReader(open(src / "codes.csv", encoding="utf-8")):
    codes.setdefault(c["Parameter_ID"], []).append(f'{c["Name"]}={c["Description"]}')
order = {f["ID"]: i for i, f in enumerate(features)}
values = {}
for v in csv.DictReader(open(src / "values.csv", encoding="utf-8")):
    row = values.setdefault(v["Language_ID"], ["."] * len(features))
    value = v["Value"]
    assert len(value) == 1, value
    row[order[v["Parameter_ID"]]] = value
with open(out / "features.tsv", "w", encoding="utf-8", newline="\n") as f:
    f.write("id\tquestion\tcodes\n")
    for x in features:
        q = " ".join(x["Name"].split())
        f.write(f'{x["ID"]}\t{q}\t{"|".join(codes.get(x["ID"], []))}\n')
with open(out / "values.tsv", "w", encoding="utf-8", newline="\n") as f:
    f.write("glottocode\tvalues\n")
    for lang in sorted(values):
        f.write(f"{lang}\t{''.join(values[lang])}\n")
print(len(features), "features,", len(values), "languages")
