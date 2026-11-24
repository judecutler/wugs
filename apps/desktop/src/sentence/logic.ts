// The Sentence panel's drawing (SCROLLS.md §1 "Correct where you look"): a
// dependency tree as arcs over the words, the way brat, UD's own viewer
// and Arborator draw it. Pure: no DOM.

import { latexEscape } from "../gloss/example.ts";

export type WordRow = { token: string; form: string; lemma: string; upos: string; xpos: string; feats: string; head: number | null; deprel: string };
export type SentenceView = { segmentId: string; content: string; words: WordRow[]; annotated: boolean; emptyNodes: number; problems: string[]; machine: string | null };

/** How far a text's tagging is checked (src-tauri/src/sentence.rs). */
export type CheckView = { analysed: number; unchecked: string[]; sentences: string[] };

/** The next sentence still to check after `current` in text order, wrapping
 * round; `current` itself only when it is the last one left. */
export function nextUnchecked(v: CheckView, current: string | null): string | null {
  if (v.unchecked.length === 0) return null;
  const at = current === null ? -1 : v.sentences.indexOf(current);
  const place = (id: string) => v.sentences.indexOf(id);
  const later = v.unchecked.find((id) => place(id) > at && id !== current);
  return later ?? v.unchecked.find((id) => id !== current) ?? v.unchecked[0] ?? null;
}

/** The line about checking a text's tags. */
export function checkLine(v: CheckView): string {
  const left = v.unchecked.length;
  if (v.analysed === 0) return "";
  if (left === 0) return `All ${v.analysed} tagged ${v.analysed === 1 ? "sentence is" : "sentences are"} checked.`;
  return `${left} of ${v.analysed} tagged ${v.analysed === 1 ? "sentence" : "sentences"} still to check.`;
}

/** The note on a sentence whose tags no one has checked. */
export function machineNote(by: string): string {
  return `Tagged by ${by}, not checked yet. Correct anything wrong below; Mark checked when it reads right.`;
}

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

/** A word or relation for tikz-dependency: LaTeX's specials escaped, and a
 * literal `&` written by its character code, since `\&` is the column
 * separator inside `deptext`. */
const tikzWord = (s: string): string => (s === "" ? "{}" : latexEscape(s.replace(/&/gu, "\u0000")).replace(/\u0000/gu, "{\\char38}"));

/** The tree as a tikz-dependency figure (`\usepackage{tikz-dependency}`):
 * the words over their UPOS, an edge from each head to its dependent with
 * its relation, `\deproot` for the root. Words with no head get no edge. */
export function tikzDependency(v: Pick<SentenceView, "content" | "words">): string {
  const n = v.words.length;
  const row = (cells: string[]) => `    ${cells.join(" \\& ")} \\\\`;
  const lines = [`% ${v.content.replace(/[\r\n]+/gu, " ")}`, "% \\usepackage{tikz-dependency}", "\\begin{dependency}", "  \\begin{deptext}"];
  lines.push(row(v.words.map((w) => tikzWord(w.form))));
  if (v.words.some((w) => w.upos !== "")) lines.push(row(v.words.map((w) => tikzWord(w.upos))));
  lines.push("  \\end{deptext}");
  v.words.forEach((w, i) => {
    if (w.head === 0) lines.push(`  \\deproot{${i + 1}}{${tikzWord(w.deprel || "root")}}`);
    else if (w.head !== null && w.head >= 1 && w.head <= n && w.head !== i + 1) lines.push(`  \\depedge{${w.head}}{${i + 1}}{${tikzWord(w.deprel)}}`);
  });
  lines.push("\\end{dependency}");
  return lines.join("\n") + "\n";
}

/** The sentence as CoNLL-U: `# text`, then a row per word with what the
 * panel shows (DEPS and MISC left `_`). Empty cells are `_`. */
export function conlluOf(v: Pick<SentenceView, "content" | "words">): string {
  const cell = (s: string) => (s === "" ? "_" : s.replace(/[\t\r\n]+/gu, " "));
  const rows = v.words.map((w, i) => [String(i + 1), cell(w.form), cell(w.lemma), cell(w.upos), cell(w.xpos), cell(w.feats), w.head === null ? "_" : String(w.head), cell(w.deprel), "_", "_"].join("\t"));
  return [`# text = ${v.content.replace(/[\r\n]+/gu, " ")}`, ...rows, "", ""].join("\n");
}
