# wugs's adapter for spaCy (the `morphosyntax` capability's `spacy`
# provider; src/morphosyntax.rs). Run as `python -c <this> <pipeline>`:
# reads one sentence a line on stdin (UTF-8), writes one CoNLL-U sentence per
# line on stdout, and the pipeline's name and version on stderr. Each line is
# kept one sentence (sentence splitters left out, every token after the first
# marked as not starting one), so the sentence count always matches.
import sys

# `-c` puts the working folder first on the path; nothing is imported from it.
# (Not `-I`, which would also hide a spaCy installed with `pip install --user`.)
sys.path[:] = [p for p in sys.path if p not in ("", ".")]

import spacy
from spacy.language import Language

@Language.component("wugs_one_sentence")
def _one_sentence(doc):
    for t in doc[1:]:
        t.is_sent_start = False
    return doc

def clean(s):
    s = s.replace("\t", " ").replace("\n", " ").replace("\r", " ")
    return s if s.strip() != "" else "_"

def main():
    name = sys.argv[1]
    nlp = spacy.load(name, exclude=["senter", "sentencizer"])
    nlp.add_pipe("wugs_one_sentence", first=True)
    lines = sys.stdin.buffer.read().decode("utf-8").split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    out = []
    for doc in nlp.pipe(lines):
        parsed = doc.has_annotation("DEP")
        toks = [t for t in doc if not t.is_space]
        num = {t.i: k + 1 for k, t in enumerate(toks)}
        out.append("# text = " + clean(doc.text))
        for k, t in enumerate(toks):
            head, rel = "_", "_"
            if parsed:
                h = 0 if t.head.i == t.i else num.get(t.head.i)
                if h is not None:
                    head = str(h)
                    rel = "root" if h == 0 else clean(t.dep_)
            misc = "SpaceAfter=No" if k + 1 < len(toks) and t.whitespace_ == "" else "_"
            out.append("\t".join([str(k + 1), clean(t.text), clean(t.lemma_), clean(t.pos_), clean(t.tag_), clean(str(t.morph)), head, rel, "_", misc]))
        out.append("")
    sys.stdout.buffer.write(("\n".join(out) + "\n").encode("utf-8"))
    meta = nlp.meta
    sys.stderr.write("wugs-pipeline: %s_%s %s\n" % (meta.get("lang", ""), meta.get("name", name), meta.get("version", "")))

main()
