// The Concordance's Plot view (AntConc's Plot): where in each text an item
// occurs, and how evenly it is spread over the texts. Pure: no DOM.

import { COORDINATE_KEYS } from "../progress/places.ts";

export type TextPlot = { textId: string; title: string; words: number; hits: number; positions: number[]; meta?: [string, string][] };
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
/** A normalised DP in words. */
export const spreadReading = (norm: number): string =>
  norm < 0.2 ? "spread evenly across the texts" : norm < 0.5 ? "spread unevenly" : norm < 0.8 ? "concentrated in a few texts" : "almost all in one text";

export function spreadWords(d: Dispersion): string {
  const parts = d.texts.filter((t) => t.words > 0).length;
  const found = d.texts.filter((t) => t.hits > 0).length;
  const where = `Found ${d.hits} ${d.hits === 1 ? "time" : "times"} in ${found} of ${parts} ${parts === 1 ? "text" : "texts"} (${d.words} running words).`;
  if (d.dp === null) return d.hits === 0 ? "Not found." : `${where} Spread needs two texts or more.`;
  const norm = d.dpNorm ?? d.dp;
  return `${where} DP ${d.dp.toFixed(2)} (normalised ${norm.toFixed(2)}): ${spreadReading(norm)}. 0 means in step with each text's size, 1 all in one text.`;
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

/** The details the plotted texts carry, by how many texts have each, most first; coordinates are for the map. */
export function plotDetails(texts: readonly TextPlot[]): string[] {
  const n = new Map<string, number>();
  for (const t of texts) for (const [k] of t.meta ?? []) if (!COORDINATE_KEYS.includes(k)) n.set(k, (n.get(k) ?? 0) + 1);
  return [...n].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).map(([k]) => k);
}

/** Texts without the detail, grouped last. */
export const NOT_GIVEN = "(not given)";

export type DetailRow = { value: string; texts: number; words: number; hits: number };

/** The matches summed per value of one detail (genre, speaker…), the highest
 * rate first and texts without it last: how an item varies across the
 * corpus's parts. */
export function byDetail(texts: readonly TextPlot[], key: string): DetailRow[] {
  const rows = new Map<string, DetailRow>();
  for (const t of texts) {
    const value = (t.meta ?? []).find(([k]) => k === key)?.[1] ?? NOT_GIVEN;
    const r = rows.get(value) ?? { value, texts: 0, words: 0, hits: 0 };
    r.texts += 1;
    r.words += t.words;
    r.hits += t.hits;
    rows.set(value, r);
  }
  const rate = (r: DetailRow) => (r.words === 0 ? 0 : r.hits / r.words);
  return [...rows.values()].sort((a, b) => Number(a.value === NOT_GIVEN) - Number(b.value === NOT_GIVEN) || rate(b) - rate(a) || a.value.localeCompare(b.value));
}

/** The rows as a table for a spreadsheet. */
export function detailTsv(rows: readonly DetailRow[], key: string): string {
  return [[key, "texts", "words", "matches", "per 1000 words"].join("\t"), ...rows.map((r) => [r.value, r.texts, r.words, r.hits, per1000(r.hits, r.words)].join("\t"))].join("\n") + "\n";
}

/** The year a detail's value names: its first number of 3 or 4 digits
 * (`1953`, `1953-04-02`, `c. 850`), negative with BC or BCE after it; null
 * when it names none. */
export function yearOf(value: string): number | null {
  const m = /(?<![\d.])(\d{3,4})(?![\d.])(\s*(?:BCE|BC|B\.C\.E?\.?))?/iu.exec(value);
  if (m === null) return null;
  const y = Number(m[1]);
  return m[2] === undefined ? y : -y;
}

/** The details at least two texts give a year in, for a plot over time. */
export function timeKeys(texts: readonly TextPlot[]): string[] {
  const n = new Map<string, number>();
  for (const t of texts) for (const [k, v] of t.meta ?? []) if (yearOf(v) !== null) n.set(k, (n.get(k) ?? 0) + 1);
  return [...n].filter(([, c]) => c >= 2).sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).map(([k]) => k);
}

export type PeriodRow = { start: number; end: number; label: string; texts: number; words: number; hits: number };
export type Periods = { width: number; rows: PeriodRow[]; undated: number };

const yearLabel = (y: number) => (y < 0 ? `${-y} BCE` : String(y));

/** The matches summed per period of the years a detail gives, oldest first,
 * every period between the first and the last listed (empty ones too, so
 * gaps show): a year each over 30 years or less, a decade each up to 300,
 * else a century. */
export function byPeriod(texts: readonly TextPlot[], key: string): Periods {
  const dated: { year: number; t: TextPlot }[] = [];
  let undated = 0;
  for (const t of texts) {
    const v = (t.meta ?? []).find(([k]) => k === key)?.[1];
    const year = v === undefined ? null : yearOf(v);
    if (year === null) undated += 1;
    else dated.push({ year, t });
  }
  if (dated.length === 0) return { width: 1, rows: [], undated };
  const lo = Math.min(...dated.map((d) => d.year));
  const hi = Math.max(...dated.map((d) => d.year));
  const width = hi - lo <= 30 ? 1 : hi - lo <= 300 ? 10 : 100;
  const first = Math.floor(lo / width) * width;
  const rows: PeriodRow[] = [];
  for (let s = first; s <= hi; s += width) {
    const e = s + width - 1;
    rows.push({ start: s, end: e, label: width === 1 ? yearLabel(s) : width === 10 && s >= 0 ? `${s}s` : `${yearLabel(s)}–${yearLabel(e)}`, texts: 0, words: 0, hits: 0 });
  }
  for (const d of dated) {
    const r = rows[Math.floor((d.year - first) / width)]!;
    r.texts += 1;
    r.words += d.t.words;
    r.hits += d.t.hits;
  }
  return { width, rows, undated };
}

export function periodTsv(p: Periods, key: string): string {
  return [[`period (${key})`, "from", "to", "texts", "words", "matches", "per 1000 words"].join("\t"), ...p.rows.map((r) => [r.label, r.start, r.end, r.texts, r.words, r.hits, r.words === 0 ? "" : per1000(r.hits, r.words)].join("\t"))].join("\n") + "\n";
}
