"""Build datasets/wals/ from a clone of https://github.com/cldf-datasets/wals.

    git clone --depth 1 https://github.com/cldf-datasets/wals /tmp/wals
    python3 tools/data/wals.py /tmp/wals datasets/wals

Written in the same shape as datasets/grambank/ (tools/data/grambank.py), so
one reader serves both. features.tsv: one row per feature in WALS's order:
id, name, codes ("1=Small|2=Moderately small"); a code's number is written as
one character, 1-9 then a-z for 10 and up. values.tsv: one row per Glottocode
with one character per feature, `.` where it is not coded, and the WALS code
of its languoid with the most values (for its page at wals.info). WALS sometimes has
two languoids for one Glottocode (dialects it keeps apart); the one with more
values comes first and the other fills its gaps.
"""
import csv
import sys
from pathlib import Path

DIGITS = "0123456789abcdefghijklmnopqrstuvwxyz"
src, out = Path(sys.argv[1]) / "cldf", Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
features = list(csv.DictReader(open(src / "parameters.csv", encoding="utf-8")))
order = {f["ID"]: i for i, f in enumerate(features)}
codes = {}
for c in csv.DictReader(open(src / "codes.csv", encoding="utf-8")):
    codes.setdefault(c["Parameter_ID"], []).append((int(c["Number"]), " ".join(c["Name"].split())))
glotto = {l["ID"]: l["Glottocode"] for l in csv.DictReader(open(src / "languages.csv", encoding="utf-8")) if l["Glottocode"]}
by_lang = {}
for v in csv.DictReader(open(src / "values.csv", encoding="utf-8")):
    row = by_lang.setdefault(v["Language_ID"], ["."] * len(features))
    row[order[v["Parameter_ID"]]] = DIGITS[int(v["Value"])]
merged = {}
first_id = {}
for lang in sorted(by_lang, key=lambda l: -sum(c != "." for c in by_lang[l])):
    code = glotto.get(lang)
    if code is None:
        continue
    row = merged.setdefault(code, ["."] * len(features))
    first_id.setdefault(code, lang)
    for i, c in enumerate(by_lang[lang]):
        if row[i] == ".":
            row[i] = c
with open(out / "features.tsv", "w", encoding="utf-8", newline="\n") as f:
    f.write("id\tquestion\tcodes\n")
    for x in features:
        cs = "|".join(f"{DIGITS[n]}={name}" for n, name in sorted(codes.get(x["ID"], [])))
        f.write(f'{x["ID"]}\t{" ".join(x["Name"].split())}\t{cs}\n')
with open(out / "values.tsv", "w", encoding="utf-8", newline="\n") as f:
    f.write("glottocode\tvalues\twals\n")
    for code in sorted(merged):
        f.write(f"{code}\t{''.join(merged[code])}\t{first_id[code]}\n")
print(len(features), "features,", len(merged), "languages")
