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
export function groupTitle(g: Pick<SketchGroup, "relation" | "direction">, lemma: string): string {
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

// ------------------------------------------------------------ sketch difference

export type DiffItem = { lemma: string; a: SketchItem | null; b: SketchItem | null; diff: number };
export type DiffGroup = { relation: string; direction: "dependent" | "head"; total: number; items: DiffItem[] };

/** Two lemmas' sketches side by side, as Sketch Engine's sketch difference:
 * per relation, every collocate of either with each one's logDice; the
 * collocates most typical of the first lemma first, of the second last.
 * Relations with most uses together first. */
export function sketchDiff(a: WordSketch, b: WordSketch): DiffGroup[] {
  const key = (g: SketchGroup) => `${g.direction}\u0001${g.relation}`;
  const groups = new Map<string, { relation: string; direction: "dependent" | "head"; total: number; a: SketchItem[]; b: SketchItem[] }>();
  for (const [side, w] of [["a", a], ["b", b]] as const) {
    for (const g of w.groups) {
      const k = key(g);
      const at = groups.get(k) ?? { relation: g.relation, direction: g.direction, total: 0, a: [], b: [] };
      at.total += g.total;
      at[side].push(...g.items);
      groups.set(k, at);
    }
  }
  return [...groups.values()]
    .map((g) => {
      const lemmas = new Set([...g.a.map((i) => i.lemma), ...g.b.map((i) => i.lemma)]);
      const items = [...lemmas].map((lemma): DiffItem => {
        const x = g.a.find((i) => i.lemma === lemma) ?? null;
        const y = g.b.find((i) => i.lemma === lemma) ?? null;
        return { lemma, a: x, b: y, diff: (x?.logDice ?? 0) - (y?.logDice ?? 0) };
      });
      items.sort((p, q) => q.diff - p.diff || p.lemma.localeCompare(q.lemma));
      return { relation: g.relation, direction: g.direction, total: g.total, items };
    })
    .sort((p, q) => q.total - p.total || p.relation.localeCompare(q.relation));
}

/** Which lemma a collocate leans to: "a", "b", or "both" when the scores are within 1 logDice. */
export const leaning = (i: DiffItem): "a" | "b" | "both" => (i.a !== null && i.b !== null && Math.abs(i.diff) < 1 ? "both" : i.diff > 0 ? "a" : "b");

/** The line above a sketch difference. */
export function diffLine(a: WordSketch, b: WordSketch, groups: readonly DiffGroup[]): string {
  if (a.lemma === "" || b.lemma === "") return `${[a, b].filter((w) => w.lemma === "").length === 2 ? "Neither lemma is" : "One lemma is not"} in the parsed sentences.`;
  const items = groups.flatMap((g) => g.items);
  const shared = items.filter((i) => leaning(i) === "both").length;
  return `«${a.lemma}» (${a.count}) against «${b.lemma}» (${b.count}): ${items.length} collocates in ${groups.length} relations, ${shared} used alike. Those typical of «${a.lemma}» first, of «${b.lemma}» last.`;
}

/** Tab-separated, a row per relation and collocate. */
export function diffTsv(a: WordSketch, b: WordSketch, groups: readonly DiffGroup[]): string {
  const head = ["relation", "direction", "lemma", `${a.lemma} count`, `${a.lemma} logDice`, `${b.lemma} count`, `${b.lemma} logDice`, "difference"].join("\t");
  const rows = groups.flatMap((g) => g.items.map((i) => [g.relation, g.direction, i.lemma, i.a?.count ?? 0, i.a?.logDice.toFixed(2) ?? "", i.b?.count ?? 0, i.b?.logDice.toFixed(2) ?? "", i.diff.toFixed(2)].join("\t")));
  return [head, ...rows].join("\n");
}
