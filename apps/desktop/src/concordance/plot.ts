// The Concordance's Plot view (AntConc's Plot): where in each text an item
// occurs, and how evenly it is spread over the texts. Pure: no DOM.

export type TextPlot = { textId: string; title: string; words: number; hits: number; positions: number[] };
export type Dispersion = { texts: TextPlot[]; words: number; hits: number; dp: number | null; dpNorm: number | null };

/** The plot's columns: each position's place along a bar of `columns`
 * columns, one mark per column however many matches fall in it. */
export function marks(positions: readonly number[], words: number, columns: number): number[] {
  if (words <= 0 || columns <= 0) return [];
  const out = new Set<number>();
  for (const p of positions) out.add(Math.min(columns - 1, Math.floor((p / words) * columns)));
  return [...out].sort((a, b) => a - b);
}

/** Matches per 1000 running words, one decimal. */
export const per1000 = (hits: number, words: number): string => (words === 0 ? "0" : ((hits / words) * 1000).toFixed(1));

/** DP in words: what the number means to someone who has not read Gries. */
export function spreadWords(d: Dispersion): string {
  const parts = d.texts.filter((t) => t.words > 0).length;
  const found = d.texts.filter((t) => t.hits > 0).length;
  const where = `Found ${d.hits} ${d.hits === 1 ? "time" : "times"} in ${found} of ${parts} ${parts === 1 ? "text" : "texts"} (${d.words} running words).`;
  if (d.dp === null) return d.hits === 0 ? "Not found." : `${where} Spread needs two texts or more.`;
  const norm = d.dpNorm ?? d.dp;
  const reading = norm < 0.2 ? "spread evenly across the texts" : norm < 0.5 ? "spread unevenly" : norm < 0.8 ? "concentrated in a few texts" : "almost all in one text";
  return `${where} DP ${d.dp.toFixed(2)} (normalised ${norm.toFixed(2)}): ${reading}. 0 means in step with each text's size, 1 all in one text.`;
}

export type PlotSort = "text" | "hits" | "rate";

export const PLOT_SORTS: { id: PlotSort; label: string }[] = [
  { id: "text", label: "Texts in order" },
  { id: "hits", label: "Most matches first" },
  { id: "rate", label: "Highest rate first" },
];

/** The rows in the chosen order; texts without a match last unless in text order. */
export function sortPlots(texts: readonly TextPlot[], by: PlotSort): TextPlot[] {
  const rate = (t: TextPlot) => (t.words === 0 ? 0 : t.hits / t.words);
  const out = texts.filter((t) => t.words > 0);
  if (by === "hits") out.sort((a, b) => b.hits - a.hits || rate(b) - rate(a));
  else if (by === "rate") out.sort((a, b) => rate(b) - rate(a) || b.hits - a.hits);
  return out;
}

/** The plot as a table for a spreadsheet: text, words, matches, per 1000, positions. */
export function plotTsv(d: Dispersion): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  const rows = d.texts.map((t) => [clean(t.title), t.words, t.hits, per1000(t.hits, t.words), t.positions.join(" ")].join("\t"));
  return ["Text\tWords\tMatches\tPer 1000\tWord positions", ...rows].join("\n") + "\n";
}
