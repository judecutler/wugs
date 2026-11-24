// Collexeme analysis (src-tauri/src/collexemes.rs): words and display rules. Pure.
import { cqpLiteral } from "../frequencies/logic.ts";

export type CollexemeRow = { item: string; together: number; freq: number; expected: number; strength: number; logLikelihood: number };
export type Collexemes = { rows: CollexemeRow[]; constructions: number; tokens: number; types: number; positions: number };

/** Strength as Gries prints it; past 300 the exact figure no longer matters. */
export const strengthCell = (v: number): string => (Math.abs(v) >= 300 ? (v > 0 ? "> 300" : "< −300") : v.toFixed(2).replace("-", "−"));

/** −log10 p of 1.3 is p < .05 (one-tailed). */
export const SIGNIFICANT = 1.3;

/** The line above the table. */
export function collexemeNote(c: Collexemes, query: string, slot: number, lemma: boolean): string {
  const n = (x: number) => x.toLocaleString("en");
  if (c.tokens === 0) return "No tagged sentences here yet.";
  if (c.constructions === 0) return c.positions > 0 && slot > c.positions ? `The query's matches have ${c.positions} ${c.positions === 1 ? "position" : "positions"}, so position ${slot} holds nothing.` : `«${query}» matches nothing here.`;
  const sig = c.rows.filter((r) => r.strength >= SIGNIFICANT).length;
  return `«${query}» matches ${n(c.constructions)} ${c.constructions === 1 ? "time" : "times"} in ${n(c.tokens)} words; position ${slot} holds ${n(c.types)} different ${lemma ? "lemmas" : "words"}, ${n(sig)} drawn to it beyond chance (strength 1.3 or more, p < .05). Strength is −log10 of Fisher's exact p (Stefanowitsch and Gries 2003); below zero, the word keeps away from the slot.`;
}

export function collexemeTsv(c: Collexemes): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  return ["collexeme\tin the slot\tfrequency\texpected\tcollostruction strength\tlog-likelihood", ...c.rows.map((r) => [clean(r.item), r.together, r.freq, r.expected.toFixed(2), r.strength.toFixed(3), r.logLikelihood.toFixed(2)].join("\t"))].join("\n") + "\n";
}

/** The query with position `slot` (from 1) narrowed to one word or lemma, so a row opens just its lines; null when the query's tokens cannot be told apart. */
export function narrowed(query: string, slot: number, item: string, lemma: boolean): string | null {
  const meta = query.indexOf("::");
  const body = meta === -1 ? query : query.slice(0, meta);
  const tail = meta === -1 ? "" : query.slice(meta);
  // Tokens are bracketed specs or quoted words; repeats and groups make positions ambiguous.
  if (/[()?*+{}|]/u.test(body.replace(/"(?:[^"\\]|\\.)*"/gu, '""'))) return null;
  const tokens = body.match(/\[[^\]]*\]|"(?:[^"\\]|\\.)*"/gu);
  if (tokens === null || slot < 1 || slot > tokens.length) return null;
  const literal = cqpLiteral(item);
  const test = `${lemma ? "lemma" : "word"}="${literal}" %c`;
  const t = tokens[slot - 1]!;
  const inner = t.startsWith("[") ? t.slice(1, -1).trim() : `word="${t.slice(1, -1)}"`;
  tokens[slot - 1] = inner === "" ? `[${test}]` : `[${inner} & ${test}]`;
  return tokens.join(" ") + (tail === "" ? "" : ` ${tail}`);
}
