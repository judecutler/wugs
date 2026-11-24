"""Build datasets/phoible/ from PHOIBLE's phoible.csv.

    curl -LO https://raw.githubusercontent.com/phoible/dev/master/data/phoible.csv
    python3 -I tools/data/phoible.py phoible.csv datasets/phoible

inventories.tsv: one row per inventory with a Glottocode: inventory id,
Glottocode, source (contributor database), language name as the source
gives it, and its phonemes in PHOIBLE's order, space-separated, in NFC.
Marginal phonemes are left out; allophones and features stay in PHOIBLE.
"""
import csv
import sys
import unicodedata
from pathlib import Path

src, out = Path(sys.argv[1]), Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
inventories = {}
csv.field_size_limit(1 << 20)
with open(src, encoding="utf-8", newline="") as f:
    for row in csv.DictReader(f):
        code = row["Glottocode"].strip()
        if code in ("", "NA"):
            continue
        inv = inventories.setdefault(row["InventoryID"], {"code": code, "source": row["Source"], "name": row["LanguageName"].strip(), "phonemes": []})
        if row["Marginal"].strip().upper() == "TRUE":
            continue
        p = unicodedata.normalize("NFC", row["Phoneme"].strip())
        assert p and " " not in p and "\t" not in p, p
        if p not in inv["phonemes"]:
            inv["phonemes"].append(p)
with open(out / "inventories.tsv", "w", encoding="utf-8", newline="\n") as f:
    f.write("id\tglottocode\tsource\tname\tphonemes\n")
    for i, inv in sorted(inventories.items(), key=lambda kv: int(kv[0])):
        name = inv["name"].replace("\t", " ")
        f.write(f'{i}\t{inv["code"]}\t{inv["source"]}\t{name}\t{" ".join(inv["phonemes"])}\n')
print(len(inventories), "inventories")
