// Word sketch (src-tauri/src/wordsketch.rs): a lemma's company by
// grammatical relation, read from the dependency trees. Pure: the words for
// each relation, the line above, the query a collocate opens and the copy.

import { cqpLiteral } from "../frequencies/logic.ts";

export type SketchItem = { lemma: string; count: number; logDice: number; after: number };
export type SketchGroup = { relation: string; direction: "dependent" | "head"; total: number; items: SketchItem[] };
export type WordSketch = { lemma: string; count: number; parsed: number; groups: SketchGroup[] };

/** What a UD relation's dependents are, in words (base relation, before any `:`). */
const DEPENDENTS: Record<string, string> = {
  nsubj: "subjects",
  obj: "objects",
  iobj: "indirect objects",
  csubj: "clausal subjects",
  ccomp: "clausal complements",
  xcomp: "open complements",
  obl: "obliques",
  vocative: "vocatives",
  expl: "expletives",
  dislocated: "dislocated words",
  advcl: "adverbial clauses",
  advmod: "adverbs",
  discourse: "discourse words",
  aux: "auxiliaries",
  cop: "copulas",
  mark: "subordinators",
  nmod: "noun modifiers",
  appos: "appositions",
  nummod: "numbers",
  acl: "clauses modifying it",
  amod: "adjectives",
  det: "determiners",
  clf: "classifiers",
  case: "case markers and adpositions",
  conj: "words coordinated with it",
  cc: "conjunctions",
  fixed: "fixed expressions",
  flat: "flat names",
  compound: "compounds",
  list: "list items",
  parataxis: "parataxis",
  orphan: "orphans",
  goeswith: "parts of the same word",
  reparandum: "repairs",
  dep: "unspecified dependents",
};

/** A group's title: "objects of «eat»", or "words «dog» is the nsubj of". */
export function groupTitle(g: SketchGroup, lemma: string): string {
  const base = g.relation.split(":")[0]!;
  const sub = g.relation.includes(":") ? ` (${g.relation})` : "";
  if (g.direction === "dependent") {
    const what = DEPENDENTS[base];
    return what === undefined ? `${g.relation} of «${lemma}»` : `${what[0]!.toUpperCase()}${what.slice(1)} of «${lemma}»${sub}`;
  }
  if (base === "conj") return `Coordinated with «${lemma}»${sub}`;
  const what = DEPENDENTS[base];
  return what === undefined ? `Words «${lemma}» is the ${g.relation} of` : `Words «${lemma}» is among the ${what} of${sub}`;
}

/** The line above the groups. */
export function sketchLine(w: WordSketch, asked: string): string {
  if (w.parsed === 0) return "No sentence is parsed yet: tag and parse texts in the Sentence panel, or import CoNLL-U or TEI with dependencies.";
  if (w.lemma === "") return `«${asked.trim()}» is not a lemma in the ${w.parsed.toLocaleString("en")} parsed sentences.`;
  const n = w.count.toLocaleString("en");
  return `«${w.lemma}»: ${n} ${w.count === 1 ? "use" : "uses"} in ${w.parsed.toLocaleString("en")} parsed sentences, ${w.groups.length} ${w.groups.length === 1 ? "relation" : "relations"}. Strongest first by logDice (14 at most: the two always together).`;
}

/** The CQP query that lists the lines with both lemmas, in the order they mostly stand, up to ten words apart. */
export function pairQuery(node: string, item: SketchItem): string {
  const a = `[lemma="${cqpLiteral(node)}" %c]`;
  const b = `[lemma="${cqpLiteral(item.lemma)}" %c]`;
  return item.after * 2 >= item.count ? `${a} []{0,10} ${b}` : `${b} []{0,10} ${a}`;
}

/** Tab-separated, a row per collocate. */
export function sketchTsv(w: WordSketch): string {
  const head = ["relation", "direction", "lemma", "count", "logDice"].join("\t");
  const rows = w.groups.flatMap((g) => g.items.map((i) => [g.relation, g.direction, i.lemma, i.count, i.logDice.toFixed(2)].join("\t")));
  return [head, ...rows].join("\n");
}
