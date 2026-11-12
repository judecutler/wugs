// Collocates (Scrolls): display rules. Pure: no DOM.

export type CollocateRow = {
  item: string;
  together: number;
  left: number;
  right: number;
  freq: number;
  logLikelihood: number;
  mutualInformation: number;
};
export type Collocates = { rows: CollocateRow[]; node: number; tokens: number; types: number };

export type CollocateSort = "ll" | "mi" | "together" | "alpha";

export const COLLOCATE_SORTS: { id: CollocateSort; label: string }[] = [
  { id: "ll", label: "Strongest first (log-likelihood)" },
  { id: "mi", label: "Most exclusive first (mutual information)" },
  { id: "together", label: "Most often together first" },
  { id: "alpha", label: "Alphabetical" },
];

/** Window sizes on each side, as AntConc offers them. */
export const SPANS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] as const;

const collator = new Intl.Collator(undefined, { sensitivity: "accent", numeric: true });

/** Sorted copy; the backend's order (log-likelihood) breaks ties. */
export function sortCollocates(rows: readonly CollocateRow[], by: CollocateSort): CollocateRow[] {
  const out = [...rows];
  if (by === "mi") return out.sort((a, b) => b.mutualInformation - a.mutualInformation || b.together - a.together);
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
  const listed = c.rows.length < c.types ? ` ${n(c.rows.length)} are listed (raise or lower the minimum to change that).` : "";
  return `«${node}» occurs ${n(c.node)} ${c.node === 1 ? "time" : "times"} in ${n(c.tokens)} words; ${n(c.types)} different words keep it company within ${span}.${listed}`;
}

/** Tab-separated with a header row, for a spreadsheet. */
export function asTsv(rows: readonly CollocateRow[]): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  return [
    "collocate\ttogether\tleft\tright\tfrequency\tlog-likelihood\tmutual information",
    ...rows.map((r) => [clean(r.item), r.together, r.left, r.right, r.freq, score(r.logLikelihood), score(r.mutualInformation)].join("\t")),
  ].join("\n");
}
