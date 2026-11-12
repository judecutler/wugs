// Frequency list (Scrolls): display rules. Pure: no DOM.

export type FrequencyRow = { item: string; count: number; texts: number };
/** A row with its frequency rank (1 = most frequent), kept through sorting and filtering. */
export type RankedRow = FrequencyRow & { rank: number };

/** Ranks in the backend's order (most frequent first, ties as first met). */
export const ranked = (rows: readonly FrequencyRow[]): RankedRow[] => rows.map((r, i) => ({ ...r, rank: i + 1 }));
/** `words` is the running word count rates are taken from; `once` the sequences met once and left out (n-grams). */
export type Frequencies = { rows: FrequencyRow[]; tokens: number; types: number; words?: number; once?: number };

export type FreqSort = "count" | "alpha" | "texts";

export const FREQ_SORTS: { id: FreqSort; label: string }[] = [
  { id: "count", label: "Most frequent first" },
  { id: "alpha", label: "Alphabetical" },
  { id: "texts", label: "In most texts first" },
];

const collator = new Intl.Collator(undefined, { sensitivity: "accent", numeric: true });

/** Sorted copy; ties keep the backend's order (most frequent, then first met). */
export function sortRows<R extends FrequencyRow>(rows: readonly R[], by: FreqSort): R[] {
  if (by === "count") return [...rows];
  if (by === "texts") return [...rows].sort((a, b) => b.texts - a.texts || b.count - a.count);
  return [...rows].sort((a, b) => collator.compare(a.item, b.item));
}

/** Occurrences per thousand, as corpus tools print it: one decimal under 10, none above. */
export function perThousand(count: number, tokens: number): string {
  if (tokens === 0) return "0";
  const v = (count * 1000) / tokens;
  return v >= 10 ? Math.round(v).toLocaleString("en") : v.toFixed(1);
}

/** Rows whose item contains the filter, ignoring case but not diacritics. */
export function filterRows<R extends FrequencyRow>(rows: readonly R[], filter: string): R[] {
  const f = filter.trim();
  if (f === "") return [...rows];
  const lower = f.toLocaleLowerCase();
  return rows.filter((r) => r.item.toLocaleLowerCase().includes(lower));
}

/** "1,204 words, 318 different" in the kind's own nouns. */
export function summary(f: Frequencies, noun: { one: string; many: string }): string {
  if (f.tokens === 0) return `No ${noun.many} yet.`;
  const n = (x: number) => x.toLocaleString("en");
  const shown = f.rows.length < f.types - (f.once ?? 0) ? ` The ${n(f.rows.length)} most frequent are listed.` : "";
  const once = (f.once ?? 0) > 0 ? ` The ${n(f.once!)} met only once are left out.` : "";
  return `${n(f.tokens)} ${f.tokens === 1 ? noun.one : noun.many}, ${n(f.types)} different.${once}${shown}`;
}

/** Tab-separated, with a header row, for a spreadsheet; rank is the frequency rank, whatever the order. */
export function asTsv(rows: readonly RankedRow[], tokens: number, item = "item"): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  return [`rank\t${clean(item)}\tcount\tper 1000 words\ttexts`, ...rows.map((r) => [String(r.rank), clean(r.item), String(r.count), perThousand(r.count, tokens), String(r.texts)].join("\t"))].join("\n");
}
