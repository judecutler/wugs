"""Build the concept-list JSONL files from a concepticon/concepticon-data checkout.

Usage: python3 build.py <path to concepticon-data checkout> <output directory>
Writes one file per list below: a header line (list, name, citation), then
one line per concept (id, number, gloss, concepticon, concepticonGloss).
"""
import csv, json, pathlib, sys

LISTS = [
    ("Swadesh-1955-100", "Swadesh 100 (1955)", "Swadesh, Morris. 1955. Towards greater accuracy in lexicostatistic dating."),
    ("Swadesh-1952-200", "Swadesh 200 (1952)", "Swadesh, Morris. 1952. Lexicostatistic dating of prehistoric ethnic contacts."),
    ("Tadmor-2009-100", "Leipzig-Jakarta list (Tadmor 2009)", "Tadmor, Uri, Martin Haspelmath and Bradley Taylor. 2010. Borrowability and the notion of basic vocabulary."),
]

src, out = pathlib.Path(sys.argv[1]) / "concepticondata" / "conceptlists", pathlib.Path(sys.argv[2])
for lid, name, cite in LISTS:
    rows = list(csv.DictReader((src / f"{lid}.tsv").open(encoding="utf-8"), delimiter="\t"))
    ids = [r["ID"] for r in rows]
    assert len(set(ids)) == len(ids), lid
    with (out / f"{lid}.jsonl").open("w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps({"list": lid, "name": name, "citation": cite}, ensure_ascii=False) + "\n")
        for r in rows:
            f.write(json.dumps({"id": r["ID"], "number": int(r["NUMBER"]), "gloss": r["ENGLISH"],
                                "concepticon": r["CONCEPTICON_ID"], "concepticonGloss": r["CONCEPTICON_GLOSS"]}, ensure_ascii=False) + "\n")
    print(lid, len(rows))
