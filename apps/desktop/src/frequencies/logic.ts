// Frequency list (Scrolls): display rules. Pure: no DOM.

export type FrequencyRow = { item: string; count: number; texts: number };
export type Frequencies = { rows: FrequencyRow[]; tokens: number; types: number };

export type FreqSort = "count" | "alpha" | "texts";

export const FREQ_SORTS: { id: FreqSort; label: string }[] = [
  { id: "count", label: "Most frequent first" },
  { id: "alpha", label: "Alphabetical" },
  { id: "texts", label: "In most texts first" },
];

const collator = new Intl.Collator(undefined, { sensitivity: "accent", numeric: true });

/** Sorted copy; ties keep the backend's order (most frequent, then first met). */
export function sortRows(rows: readonly FrequencyRow[], by: FreqSort): FrequencyRow[] {
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
export function filterRows(rows: readonly FrequencyRow[], filter: string): FrequencyRow[] {
  const f = filter.trim();
  if (f === "") return [...rows];
  const lower = f.toLocaleLowerCase();
  return rows.filter((r) => r.item.toLocaleLowerCase().includes(lower));
}

/** "1,204 words, 318 different" in the kind's own nouns. */
export function summary(f: Frequencies, noun: { one: string; many: string }): string {
  if (f.tokens === 0) return `No ${noun.many} yet.`;
  const n = (x: number) => x.toLocaleString("en");
  const shown = f.rows.length < f.types ? ` The ${n(f.rows.length)} most frequent are listed.` : "";
  return `${n(f.tokens)} ${f.tokens === 1 ? noun.one : noun.many}, ${n(f.types)} different.${shown}`;
}

/** Tab-separated, with a header row, for a spreadsheet. */
export function asTsv(rows: readonly FrequencyRow[], tokens: number): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  return ["rank\titem\tcount\tper 1000\ttexts", ...rows.map((r, i) => [String(i + 1), clean(r.item), String(r.count), perThousand(r.count, tokens), String(r.texts)].join("\t"))].join("\n");
}
