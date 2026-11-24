// Sorting a search's lines into categories (CQPweb's "categorise query"):
// the names, a line's key and the counts. The categories live in the
// project (src-tauri/src/hit_categories.rs). Pure, npm-tested.
import type { Hit } from "./logic.ts";

/** What the backend keeps for one search. */
export type Sorted = { categories: string[]; lines: [string, string][] };

/** A line's key: its segment and where the match starts in it. */
export const lineKey = (h: Hit): string => `${h.segmentId}:${h.at}`;

/** Category names as typed: separated by commas or line breaks, trimmed, each once. */
export function parseCategories(text: string): string[] {
  const out: string[] = [];
  for (const c of text.split(/[,\n]/u).map((x) => x.trim().replace(/\s+/gu, " "))) if (c !== "" && !out.includes(c)) out.push(c);
  return out;
}

/** Each category's lines among `hits`, in the categories' order, then lines not sorted yet. */
export function categoryCounts(hits: readonly Hit[], sorted: Sorted): { category: string; count: number }[] {
  const of = new Map(sorted.lines);
  const counts = new Map<string, number>(sorted.categories.map((c) => [c, 0]));
  let none = 0;
  for (const h of hits) {
    const c = of.get(lineKey(h));
    if (c !== undefined && counts.has(c)) counts.set(c, (counts.get(c) ?? 0) + 1);
    else none += 1;
  }
  return [...[...counts].map(([category, count]) => ({ category, count })), { category: "", count: none }];
}

/** The line over the rows: each category's share of the lines shown. */
export function categoriesLine(hits: readonly Hit[], sorted: Sorted): string {
  if (sorted.categories.length === 0) return "Type the categories to sort these lines into, separated by commas, such as literal, figurative, unclear.";
  const counts = categoryCounts(hits, sorted);
  const total = hits.length;
  const pct = (n: number) => (total === 0 ? "" : ` (${Math.round((n * 100) / total)}%)`);
  const parts = counts.filter((c) => c.category !== "").map((c) => `${c.category} ${c.count}${pct(c.count)}`);
  const left = counts[counts.length - 1]!.count;
  return `${parts.join(" · ")}${left > 0 ? ` · not sorted yet ${left}` : " · every line sorted"}.`;
}
