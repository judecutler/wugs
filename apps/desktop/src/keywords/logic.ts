// Keywords (Scrolls): display rules. Pure: no DOM.

export type KeywordRow = { item: string; focus: number; reference: number; logLikelihood: number; logRatio: number; percentDiff: number };
export type Keywords = { rows: KeywordRow[]; focusTokens: number; referenceTokens: number; types: number };

export type KeywordSort = "ll" | "ratio" | "alpha";
export const KEYWORD_SORTS: { id: KeywordSort; label: string }[] = [
  { id: "ll", label: "By log-likelihood" },
  { id: "ratio", label: "By log ratio (effect size)" },
  { id: "alpha", label: "Alphabetical" },
];

/** Which rows to show: words the text uses more, less, or both. */
export type KeywordSide = "more" | "less";

const collator = new Intl.Collator(undefined, { sensitivity: "accent", numeric: true });

/** The rows on one side, sorted; "less" puts the most negative first. */
export function keywordRows(rows: readonly KeywordRow[], side: KeywordSide, by: KeywordSort): KeywordRow[] {
  const sign = side === "more" ? 1 : -1;
  const out = rows.filter((r) => sign * r.logLikelihood > 0);
  if (by === "alpha") return out.sort((a, b) => collator.compare(a.item, b.item));
  const key = (r: KeywordRow) => sign * (by === "ratio" ? r.logRatio : r.logLikelihood);
  return out.sort((a, b) => key(b) - key(a));
}

/** Rows strong enough to count at p < 0.05 (log-likelihood ≥ 3.84). */
export const significant = (r: KeywordRow): boolean => Math.abs(r.logLikelihood) >= 3.84;

/** Per thousand words, as corpus tools print it. */
export function perThousand(count: number, tokens: number): string {
  if (tokens === 0) return "0";
  const v = (count * 1000) / tokens;
  return v >= 10 ? Math.round(v).toLocaleString("en") : v.toFixed(1);
}

export function summary(title: string, k: Keywords): string {
  const n = (x: number) => x.toLocaleString("en");
  if (k.focusTokens === 0) return `«${title}» has no words yet.`;
  if (k.referenceTokens === 0) return "There is nothing to compare with yet: keywords compare a text with the project's other texts.";
  return `«${title}» (${n(k.focusTokens)} words) against the project's other texts (${n(k.referenceTokens)} words). Rows marked • are significant at p < 0.05.`;
}

export function asTsv(rows: readonly KeywordRow[]): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  return ["word\tin text\tin rest\tlog-likelihood\tlog ratio\t%DIFF", ...rows.map((r) => [clean(r.item), r.focus, r.reference, r.logLikelihood.toFixed(2), r.logRatio.toFixed(2), r.percentDiff.toFixed(1)].join("\t"))].join("\n");
}
