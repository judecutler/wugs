// The Sentence panel's drawing (SCROLLS.md §1 "Correct where you look"): a
// dependency tree as arcs over the words, the way brat, UD's own viewer
// and Arborator draw it. Pure: no DOM.

export type WordRow = { token: string; form: string; lemma: string; upos: string; xpos: string; feats: string; head: number | null; deprel: string };
export type SentenceView = { segmentId: string; content: string; words: WordRow[]; annotated: boolean; emptyNodes: number; problems: string[] };

/** UD's universal tags and relations, offered in the editor. */
export const UPOS = ["ADJ", "ADP", "ADV", "AUX", "CCONJ", "DET", "INTJ", "NOUN", "NUM", "PART", "PRON", "PROPN", "PUNCT", "SCONJ", "SYM", "VERB", "X"];
export const DEPRELS = [
  "acl", "advcl", "advmod", "amod", "appos", "aux", "case", "cc", "ccomp", "clf", "compound", "conj", "cop", "csubj", "dep", "det", "discourse", "dislocated", "expl", "fixed", "flat", "goeswith", "iobj", "list", "mark", "nmod", "nsubj", "nummod", "obj", "obl", "orphan", "parataxis", "punct", "reparandum", "root", "vocative", "xcomp",
];

/** One arc: from the head to the dependent (word indexes from 0), with its
 * relation and how high it rides (1 = lowest). */
export type Arc = { head: number; dep: number; label: string; level: number };

/** Arcs for every word with a head other than the root, each riding just
 * above every arc it spans, so nested arcs never cross their insides. */
export function arcs(words: readonly WordRow[]): Arc[] {
  const out: Arc[] = [];
  words.forEach((w, i) => {
    if (w.head !== null && w.head > 0 && w.head <= words.length && w.head - 1 !== i) out.push({ head: w.head - 1, dep: i, label: w.deprel, level: 1 });
  });
  const span = (a: Arc) => [Math.min(a.head, a.dep), Math.max(a.head, a.dep)] as const;
  // Shorter arcs first, so an arc's level can rest on the ones under it.
  const order = [...out].sort((a, b) => span(a)[1] - span(a)[0] - (span(b)[1] - span(b)[0]));
  for (const a of order) {
    const [lo, hi] = span(a);
    let under = 0;
    for (const b of order) {
      if (b === a) break;
      const [blo, bhi] = span(b);
      if (blo >= lo && bhi <= hi) under = Math.max(under, b.level);
    }
    a.level = under + 1;
  }
  return out;
}

/** The word that is the root (index), or null. */
export const rootOf = (words: readonly WordRow[]): number | null => {
  const i = words.findIndex((w) => w.head === 0);
  return i < 0 ? null : i;
};

/** Where each word sits: its left edge and width in pixels, from the
 * longer of its form and its tag, at a monospace-ish average width. */
export function columns(words: readonly WordRow[], charPx = 8, gap = 14, min = 34): { x: number; width: number }[] {
  let x = 0;
  return words.map((w) => {
    const width = Math.max(min, Math.max([...w.form].length, w.upos.length) * charPx);
    const col = { x, width };
    x += width + gap;
    return col;
  });
}

/** The head choices for one word: the root, then every other word by number. */
export function headChoices(words: readonly WordRow[], self: number): { value: string; label: string }[] {
  return [{ value: "", label: "—" }, { value: "0", label: "0 root" }, ...words.flatMap((w, i) => (i === self ? [] : [{ value: String(i + 1), label: `${i + 1} ${w.form}` }]))];
}
