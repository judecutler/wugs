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

/** The sorted lines cross-tabulated by a grouping (text, speaker, a detail): a row per group, a column per category. */
export type CrossTab = { groups: string[]; categories: string[]; cells: number[][] };

export function crossTab(hits: readonly Hit[], sorted: Sorted, groupOf: (h: Hit) => string): CrossTab {
  const of = new Map(sorted.lines);
  const groups: string[] = [];
  const rows = new Map<string, number[]>();
  for (const h of hits) {
    const c = sorted.categories.indexOf(of.get(lineKey(h)) ?? "");
    if (c < 0) continue;
    const g = groupOf(h);
    let row = rows.get(g);
    if (row === undefined) {
      row = sorted.categories.map(() => 0);
      rows.set(g, row);
      groups.push(g);
    }
    row[c]! += 1;
  }
  groups.sort((a, b) => a.localeCompare(b));
  return { groups, categories: [...sorted.categories], cells: groups.map((g) => rows.get(g)!) };
}

/** The upper tail of the chi-square distribution: the regularized gamma Q(df/2, x/2). */
export function chiSquareP(x: number, df: number): number {
  if (x <= 0) return 1;
  const a = df / 2;
  const z = x / 2;
  const lnGamma = (v: number): number => {
    // Lanczos, g = 7.
    const c = [0.99999999999980993, 676.5203681218851, -1259.1392167224028, 771.32342877765313, -176.61502916214059, 12.507343278686905, -0.13857109526572012, 9.9843695780195716e-6, 1.5056327351493116e-7];
    if (v < 0.5) return Math.log(Math.PI / Math.sin(Math.PI * v)) - lnGamma(1 - v);
    v -= 1;
    let s = c[0]!;
    for (let i = 1; i < 9; i++) s += c[i]! / (v + i);
    const t = v + 7.5;
    return 0.5 * Math.log(2 * Math.PI) + (v + 0.5) * Math.log(t) - t + Math.log(s);
  };
  const g = lnGamma(a);
  if (z < a + 1) {
    // Series for P, then Q = 1 − P.
    let sum = 1 / a;
    let term = sum;
    for (let n = 1; n < 500; n++) {
      term *= z / (a + n);
      sum += term;
      if (Math.abs(term) < Math.abs(sum) * 1e-14) break;
    }
    return Math.max(0, 1 - sum * Math.exp(-z + a * Math.log(z) - g));
  }
  // Continued fraction for Q (Lentz).
  let b = z + 1 - a;
  let c = 1 / 1e-300;
  let d = 1 / b;
  let f = d;
  for (let i = 1; i < 500; i++) {
    const an = -i * (i - a);
    b += 2;
    d = an * d + b;
    if (Math.abs(d) < 1e-300) d = 1e-300;
    c = b + an / c;
    if (Math.abs(c) < 1e-300) c = 1e-300;
    d = 1 / d;
    const delta = d * c;
    f *= delta;
    if (Math.abs(delta - 1) < 1e-14) break;
  }
  return Math.min(1, Math.exp(-z + a * Math.log(z) - g) * f);
}

/** Pearson's chi-square test of independence on a table, with Cramér's V and the share of cells expected under 5. */
export function independence(cells: readonly (readonly number[])[]): { chi2: number; df: number; p: number; v: number; small: number } | null {
  const rows = cells.filter((r) => r.some((x) => x > 0));
  const cols = rows.length === 0 ? 0 : rows[0]!.length;
  const colSums = Array.from({ length: cols }, (_, j) => rows.reduce((a, r) => a + r[j]!, 0));
  const keep = colSums.map((s, j) => (s > 0 ? j : -1)).filter((j) => j >= 0);
  if (rows.length < 2 || keep.length < 2) return null;
  const n = rows.reduce((a, r) => a + r.reduce((x, y) => x + y, 0), 0);
  let chi2 = 0;
  let small = 0;
  for (const r of rows) {
    const rowSum = r.reduce((x, y) => x + y, 0);
    for (const j of keep) {
      const e = (rowSum * colSums[j]!) / n;
      if (e < 5) small += 1;
      chi2 += (r[j]! - e) ** 2 / e;
    }
  }
  const df = (rows.length - 1) * (keep.length - 1);
  const v = Math.sqrt(chi2 / (n * Math.min(rows.length - 1, keep.length - 1)));
  return { chi2, df, p: chiSquareP(chi2, df), v, small: small / (rows.length * keep.length) };
}

/** The line under the table, in words. */
export function independenceLine(t: ReturnType<typeof independence>, by: string): string {
  if (t === null) return `Sort lines from at least two ${by} values into at least two categories to compare them.`;
  const p = t.p < 0.001 ? "p < 0.001" : `p = ${t.p.toFixed(3)}`;
  const verdict = t.p < 0.05 ? `The categories differ by ${by} more than chance would have it` : `The categories do not differ by ${by} beyond chance`;
  const caution = t.small > 0.2 ? " Many cells expect fewer than 5 lines, so read the test with care, or sort more lines." : "";
  return `${verdict} (χ² = ${t.chi2.toFixed(2)}, df = ${t.df}, ${p}; Cramér's V = ${t.v.toFixed(2)}).${caution}`;
}

/** The table to paste. */
export function crossTabTsv(t: CrossTab, by: string): string {
  return [[by, ...t.categories, "lines"].join("\t"), ...t.groups.map((g, i) => [g, ...t.cells[i]!.map(String), String(t.cells[i]!.reduce((a, b) => a + b, 0))].join("\t"))].join("\n") + "\n";
}
