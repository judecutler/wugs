"""Build datasets/glottolog/languoids.tsv from Glottolog's CLDF languages table.

Usage: python3 -I tools/data/glottolog.py <glottolog-cldf>/cldf/languages.csv datasets/glottolog/languoids.tsv

Keeps languages and dialects (not families): glottocode, ISO 639-3 code,
name, level (L or D), the family's name (or "isolate"), the macroarea, for
a dialect the language it belongs to, and the latitude and longitude Glottolog
gives (two decimals, about a kilometre; empty when it gives none). Sorted by glottocode so a new
release diffs line by line. Glottolog is CC BY 4.0 (see the README beside
the output).
"""
import csv
import sys


def clean(s):
    return " ".join((s or "").replace("\t", " ").split())


def main(src, out):
    rows = list(csv.DictReader(open(src, encoding="utf-8")))
    names = {r["ID"]: clean(r["Name"]) for r in rows}
    kept = []
    for r in rows:
        level = r["Level"]
        if level not in ("language", "dialect"):
            continue
        family = names.get(r["Family_ID"], "") if r["Family_ID"] else ("isolate" if r["Is_Isolate"] == "True" else "")
        belongs = names.get(r["Language_ID"], "") if level == "dialect" else ""
        coord = lambda v: f"{float(v):.2f}" if v else ""
        kept.append([r["Glottocode"], r["ISO639P3code"], clean(r["Name"]), "L" if level == "language" else "D", family, clean(r["Macroarea"]), belongs, coord(r["Latitude"]), coord(r["Longitude"])])
    kept.sort(key=lambda k: k[0])
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("glottocode\tiso\tname\tlevel\tfamily\tmacroarea\tlanguage\tlatitude\tlongitude\n")
        for k in kept:
            f.write("\t".join(k) + "\n")
    print(f"{len(kept)} languoids written to {out}")


main(sys.argv[1], sys.argv[2])
