// Collocates (Scrolls): display rules. Pure: no DOM.
import { cqpLiteral } from "../frequencies/logic.ts";

export type CollocateRow = {
  item: string;
  together: number;
  left: number;
  right: number;
  freq: number;
  logLikelihood: number;
  mutualInformation: number;
  logDice: number;
  tScore: number;
};
export type Collocates = { rows: CollocateRow[]; node: number; tokens: number; types: number };

export type CollocateSort = "ll" | "dice" | "mi" | "t" | "together" | "alpha";

export const COLLOCATE_SORTS: { id: CollocateSort; label: string }[] = [
  { id: "ll", label: "By log-likelihood" },
  { id: "dice", label: "By logDice (Sketch Engine)" },
  { id: "mi", label: "By mutual information" },
  { id: "t", label: "By t-score" },
  { id: "together", label: "Most often together" },
  { id: "alpha", label: "Alphabetical" },
];

/** Window sizes on each side, as AntConc offers them. */
export const SPANS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] as const;

const collator = new Intl.Collator(undefined, { sensitivity: "accent", numeric: true });

/** Sorted copy; the backend's order (log-likelihood) breaks ties. */
export function sortCollocates(rows: readonly CollocateRow[], by: CollocateSort): CollocateRow[] {
  const out = [...rows];
  if (by === "mi") return out.sort((a, b) => b.mutualInformation - a.mutualInformation || b.together - a.together);
  if (by === "dice") return out.sort((a, b) => b.logDice - a.logDice || b.together - a.together);
  if (by === "t") return out.sort((a, b) => b.tScore - a.tScore || b.together - a.together);
  if (by === "together") return out.sort((a, b) => b.together - a.together || b.logLikelihood - a.logLikelihood);
  if (by === "alpha") return out.sort((a, b) => collator.compare(a.item, b.item));
  return out;
}

/** A score as corpus tools print it: two decimals. */
export const score = (v: number): string => v.toFixed(2);

/** "«fox» occurs 4 times; 12 words keep it company within 5 words either side." */
export function summary(node: string, c: Collocates, left: number, right: number): string {
  if (c.tokens === 0) return "No words yet.";
  if (c.node === 0) return `«${node}» does not occur here.`;
  const n = (x: number) => x.toLocaleString("en");
  const span = left === right ? `${left} ${left === 1 ? "word" : "words"} either side` : `${left} before and ${right} after`;
  const listed = c.rows.length < c.types ? ` ${n(c.rows.length)} are listed: those met together often enough, strongest by log-likelihood first (lower the minimum to see more).` : "";
  return `«${node}» occurs ${n(c.node)} ${c.node === 1 ? "time" : "times"} in ${n(c.tokens)} words; ${n(c.types)} different words keep it company within ${span}.${listed}`;
}

/** Tab-separated with a header row, for a spreadsheet. */
export function asTsv(rows: readonly CollocateRow[]): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  return [
    "collocate\ttogether\tleft\tright\tfrequency\tlog-likelihood\tmutual information\tlogDice\tt-score",
    ...rows.map((r) => [clean(r.item), r.together, r.left, r.right, r.freq, score(r.logLikelihood), score(r.mutualInformation), score(r.logDice), score(r.tScore)].join("\t")),
  ].join("\n");
}

// ------------------------------------------------------------ two words compared

/** A collocate of either word, with each one's row (null where it is no collocate of that word). */
export type CompareRow = { item: string; a: CollocateRow | null; b: CollocateRow | null; diff: number };

/**
 * Two words' collocates side by side, as Sketch Engine's word sketch
 * difference does with logDice: every collocate of either, the difference of
 * their logDice (a missing one counting as 0), those typical of the first
 * word first and of the second last. Collocates are matched case-blind.
 */
export function compareCollocates(a: readonly CollocateRow[], b: readonly CollocateRow[]): CompareRow[] {
  const key = (s: string) => s.normalize("NFC").toLocaleLowerCase();
  const rows = new Map<string, CompareRow>();
  for (const r of a) rows.set(key(r.item), { item: r.item, a: r, b: null, diff: 0 });
  for (const r of b) {
    const at = rows.get(key(r.item));
    if (at === undefined) rows.set(key(r.item), { item: r.item, a: null, b: r, diff: 0 });
    else at.b = r;
  }
  const out = [...rows.values()];
  for (const r of out) r.diff = (r.a?.logDice ?? 0) - (r.b?.logDice ?? 0);
  return out.sort((p, q) => q.diff - p.diff || collator.compare(p.item, q.item));
}

/** Which word a collocate goes with: "a", "b", or "both" when both have it within 1 logDice. */
export const leansTo = (r: CompareRow): "a" | "b" | "both" => (r.a !== null && r.b !== null && Math.abs(r.diff) < 1 ? "both" : r.diff > 0 ? "a" : "b");

/** The line above the comparison. */
export function compareLine(a: string, b: string, ca: Collocates, cb: Collocates, rows: readonly CompareRow[]): string {
  if (ca.node === 0 || cb.node === 0) return `${ca.node === 0 && cb.node === 0 ? `Neither «${a}» nor «${b}» occurs` : `«${ca.node === 0 ? a : b}» does not occur`} here.`;
  const count = (w: "a" | "b" | "both") => rows.filter((r) => leansTo(r) === w).length;
  return `«${a}» (${ca.node}) against «${b}» (${cb.node}): ${count("a")} collocates go with «${a}», ${count("b")} with «${b}» and ${count("both")} with both alike (within 1 logDice). Those typical of «${a}» first.`;
}

/** Tab-separated, for a spreadsheet. */
export function compareTsv(a: string, b: string, rows: readonly CompareRow[]): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  return [
    ["collocate", `${a} together`, `${a} logDice`, `${b} together`, `${b} logDice`, "difference"].map(clean).join("\t"),
    ...rows.map((r) => [clean(r.item), r.a?.together ?? 0, r.a === null ? "" : score(r.a.logDice), r.b?.together ?? 0, r.b === null ? "" : score(r.b.logDice), score(r.diff)].join("\t")),
  ].join("\n");
}

/**
 * The CQP query that finds a lemma pattern in the Concordance (`*` any
 * letters, `?` one, as the Collocates field reads them), capitals aside, and
 * a detail as a text condition: what a lemma row opens.
 */
export function lemmaQuery(pattern: string, within: { key: string; value: string } | null): string {
  const re = [...pattern.trim()].map((c) => (c === "*" ? ".*" : c === "?" ? "." : cqpLiteral(c))).join("");
  const q = `[lemma="${re}" %c]`;
  return within === null || within.key === "" ? q : `${q} :: match.text_${within.key}="${cqpLiteral(within.value)}"`;
}
