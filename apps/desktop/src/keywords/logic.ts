// Keywords (Scrolls): display rules. Pure: no DOM.

export type KeywordRow = { item: string; focus: number; reference: number; logLikelihood: number; logRatio: number; percentDiff: number; simpleMaths: number };
export type Keywords = { rows: KeywordRow[]; focusTokens: number; referenceTokens: number; types: number; omitted: number; critical: number };

export type KeywordSort = "ll" | "ratio" | "sm" | "alpha";
export const KEYWORD_SORTS: { id: KeywordSort; label: string }[] = [
  { id: "ll", label: "By log-likelihood" },
  { id: "ratio", label: "By log ratio (effect size)" },
  { id: "sm", label: "By simple maths (Sketch Engine)" },
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
  const key = (r: KeywordRow) => (by === "sm" ? (side === "more" ? r.simpleMaths : 1 / r.simpleMaths) : sign * (by === "ratio" ? r.logRatio : r.logLikelihood));
  return out.sort((a, b) => key(b) - key(a));
}

/** Rows significant at p < 0.05 after Bonferroni over the words tested (the backend's `critical`). */
export const significant = (r: KeywordRow, critical: number): boolean => Math.abs(r.logLikelihood) >= critical;

/** Per thousand words, as corpus tools print it. */
export function perThousand(count: number, tokens: number): string {
  if (tokens === 0) return "0";
  const v = (count * 1000) / tokens;
  return v >= 10 ? Math.round(v).toLocaleString("en") : v.toFixed(1);
}

/** The summary line; `title` is a text's title, or with `named` already words (`Texts whose genre is «news»`). */
/**
 * The line above the table. `against` names the reference when it is not the
 * rest of the project, written out ("«Hen»", "texts whose genre is «news»").
 */
export function summary(title: string, k: Keywords, named = false, against: string | null = null): string {
  const n = (x: number) => x.toLocaleString("en");
  const who = named ? title : `«${title}»`;
  if (k.focusTokens === 0) return `${who} ${named ? "have" : "has"} no words yet.`;
  if (k.referenceTokens === 0)
    return against === null ? "There is nothing to compare with yet: keywords compare a text with the project's other texts." : `There is nothing to compare with: ${against} ${against.startsWith("«") ? "has" : "have"} no words outside ${named ? title.charAt(0).toLowerCase() + title.slice(1) : `«${title}»`}.`;
  const cut = k.omitted > 0 ? ` The ${n(k.omitted)} weakest are left out.` : "";
  return `${who} (${n(k.focusTokens)} words) against ${against ?? "the project's other texts"} (${n(k.referenceTokens)} words). Rows marked • are significant at p < 0.05 after Bonferroni correction (log-likelihood ${k.critical.toFixed(2)} or more).${cut}`;
}

export function asTsv(rows: readonly KeywordRow[]): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  return ["word\tin text\tin rest\tlog-likelihood (4-term)\tlog ratio\t%DIFF\tsimple maths", ...rows.map((r) => [clean(r.item), r.focus, r.reference, r.logLikelihood.toFixed(2), r.logRatio.toFixed(2), r.percentDiff.toFixed(1), r.simpleMaths.toFixed(3)].join("\t"))].join("\n");
}
