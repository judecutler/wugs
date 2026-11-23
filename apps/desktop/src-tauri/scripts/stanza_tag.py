# wugs's adapter for Stanza (the `morphosyntax` capability's `stanza`
# provider; src/morphosyntax.rs). Run as `python -c <this> <language>`:
# reads one sentence a line on stdin (UTF-8), writes one CoNLL-U sentence per
# line on stdout, and the models it used on stderr. Each line is kept one
# sentence (`tokenize_no_ssplit`, lines sent as paragraphs), so the sentence
# count always matches. Models are never downloaded here: Stanza must have the
# language's models already (`stanza.download("<language>")`).
import sys

# `-c` puts the working folder first on the path; nothing is imported from it.
sys.path[:] = [p for p in sys.path if p not in ("", ".")]

import stanza

def clean(s):
    s = (s or "").replace("\t", " ").replace("\n", " ").replace("\r", " ")
    return s if s.strip() != "" else "_"

def main():
    lang = sys.argv[1]
    lines = sys.stdin.buffer.read().decode("utf-8").split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    settings = dict(tokenize_no_ssplit=True, download_method=None, logging_level="WARN")
    try:
        # Multi-word tokens (Spanish "del" = de + el) where the language has them.
        nlp = stanza.Pipeline(lang, processors="tokenize,mwt,pos,lemma,depparse", **settings)
    except Exception:
        nlp = stanza.Pipeline(lang, processors="tokenize,pos,lemma,depparse", **settings)
    out = []
    # Lines go as paragraphs, which `tokenize_no_ssplit` keeps whole; a blank
    # line would merge two, so it goes as "_". Should the count still differ,
    # each line is tagged on its own.
    sents = nlp("\n\n".join(l if l.strip() else "_" for l in lines)).sentences if lines else []
    if len(sents) != len(lines):
        sents = [nlp(l if l.strip() else "_").sentences[0] for l in lines]
    for line, sent in zip(lines, sents):
        out.append("# text = " + clean(line))
        for tok in sent.tokens:
            words = tok.words
            after = getattr(tok, "spaces_after", None)
            glued = after == "" if after is not None else "SpaceAfter=No" in (tok.misc or "")
            space = "SpaceAfter=No" if glued and tok is not sent.tokens[-1] else "_"
            if len(words) > 1:
                out.append("\t".join(["%d-%d" % (words[0].id, words[-1].id), clean(tok.text), "_", "_", "_", "_", "_", "_", "_", space]))
                space = "_"
            for w in words:
                head = str(w.head) if w.head is not None else "_"
                rel = clean(w.deprel) if w.head is not None else "_"
                misc = space if len(words) == 1 else "_"
                out.append("\t".join([str(w.id), clean(w.text), clean(w.lemma), clean(w.upos), clean(w.xpos), clean(w.feats), head, rel, "_", misc]))
        out.append("")
    sys.stdout.buffer.write(("\n".join(out) + "\n").encode("utf-8"))
    sys.stderr.write("wugs-pipeline: stanza %s %s\n" % (stanza.__version__, lang))

main()
