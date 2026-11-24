// Words on the rise and falling over time (src-tauri/src/trends.rs): the
// wording, the little line of each word's rate, and the table to copy. Pure.

export type Trend = { item: string; total: number; rates: (number | null)[]; tau: number };
export type Trends = { keys: string[]; key: string; labels: string[]; periodWords: number[]; undated: number; tested: number; rising: Trend[]; falling: Trend[] };

/** The polyline points of a rate line `w` by `h`, empty periods skipped; null with fewer than two points. */
export function sparkPoints(rates: readonly (number | null)[], w: number, h: number): string | null {
  const top = Math.max(0, ...rates.map((r) => r ?? 0));
  const step = rates.length > 1 ? w / (rates.length - 1) : 0;
  const pts = rates.flatMap((r, i) => (r === null ? [] : [`${(i * step).toFixed(1)},${(h - (top === 0 ? 0 : (r / top) * h)).toFixed(1)}`]));
  return pts.length < 2 ? null : pts.join(" ");
}

/** How steady a trend is, in words. */
export function steadiness(tau: number): string {
  const t = Math.abs(tau);
  return t >= 0.9 ? "every step" : t >= 0.7 ? "almost every step" : "mostly";
}

/** The first and last rates with words, for the table. */
export function fromTo(t: Trend): string {
  const known = t.rates.filter((r): r is number => r !== null);
  if (known.length === 0) return "";
  const f = (x: number) => (x >= 10 ? x.toFixed(0) : x.toFixed(1));
  return `${f(known[0]!)} → ${f(known.at(-1)!)}`;
}

/** The line above the lists. */
export function trendsLine(t: Trends, min: number): string {
  const n = (x: number, one: string, many: string) => `${x.toLocaleString("en")} ${x === 1 ? one : many}`;
  if (t.key === "") return "No detail gives a year in two texts or more. Give texts a date (or any detail with a year, such as 1953 or c. 850) in the Texts panel's Details.";
  const live = t.periodWords.filter((w) => w > 0).length;
  if (live < 3) return `The texts with a year in «${t.key}» fall in ${n(live, "period", "periods")}; trends need words in three or more.`;
  const parts = [`${n(t.tested, "word", "words")} used ${min} times or more, over ${t.labels.length} periods (${t.labels[0]} to ${t.labels.at(-1)}): ${t.rising.length} rising and ${t.falling.length} falling steadily, by their rate per 1000 words.`];
  if (t.undated > 0) parts.push(`${n(t.undated, "text has", "texts have")} no year in «${t.key}» and ${t.undated === 1 ? "is" : "are"} left out.`);
  return parts.join(" ");
}

/** Both lists as a table to paste, a column per period. */
export function trendsTsv(t: Trends): string {
  const head = ["direction", "word", "uses", "tau", ...t.labels].join("\t");
  const row = (dir: string, x: Trend) => [dir, x.item, String(x.total), x.tau.toFixed(2), ...x.rates.map((r) => (r === null ? "" : r.toFixed(2)))].join("\t");
  return [head, ...t.rising.map((x) => row("rising", x)), ...t.falling.map((x) => row("falling", x))].join("\n") + "\n";
}
