// The corpus's make-up by one detail (genre, speaker, year…): how many texts,
// sentences and words each value has, the table a corpus description in a
// paper or a deposit form gives. Pure; read from the Progress panel's texts.

import type { TextProgress } from "./logic.ts";

export type Share = { value: string; texts: number; sentences: number; words: number; glossed: number };

/** The value a text has no detail for. */
export const NONE = "(not given)";

/** Every detail key the texts use, the most widely used first. */
export function detailKeys(texts: readonly TextProgress[]): { key: string; texts: number }[] {
  const n = new Map<string, number>();
  for (const t of texts) for (const k of new Set((t.meta ?? []).map(([key]) => key))) n.set(k, (n.get(k) ?? 0) + 1);
  return [...n].map(([key, count]) => ({ key, texts: count })).sort((a, b) => b.texts - a.texts || a.key.localeCompare(b.key));
}

/** One row per value of `key`, most words first, texts without it last. */
export function composition(texts: readonly TextProgress[], key: string): Share[] {
  const rows = new Map<string, Share>();
  for (const t of texts) {
    const value = (t.meta ?? []).find(([k]) => k === key)?.[1]?.trim() || NONE;
    const r = rows.get(value) ?? { value, texts: 0, sentences: 0, words: 0, glossed: 0 };
    r.texts += 1;
    r.sentences += t.transcribed;
    r.words += t.words;
    r.glossed += t.glossedWords;
    rows.set(value, r);
  }
  return [...rows.values()].sort((a, b) => Number(a.value === NONE) - Number(b.value === NONE) || b.words - a.words || b.texts - a.texts || a.value.localeCompare(b.value));
}

/** A key as people read it: `recording_place` → "recording place". */
export const keyLabel = (key: string): string => key.replace(/_/g, " ");

/** Share of the words, as a whole percent ("—" when there are none). */
export function wordShare(r: Share, all: readonly Share[]): string {
  const total = all.reduce((n, x) => n + x.words, 0);
  if (total === 0) return "—";
  const p = (r.words * 100) / total;
  return p > 0 && p < 1 ? "<1%" : `${Math.round(p)}%`;
}

/** The line under the table. */
export function compositionNote(rows: readonly Share[], key: string): string {
  const given = rows.filter((r) => r.value !== NONE);
  const none = rows.find((r) => r.value === NONE);
  const head = `${given.length} ${given.length === 1 ? "value" : "values"} of ${keyLabel(key)}.`;
  return none === undefined ? head : `${head} ${none.texts} ${none.texts === 1 ? "text does" : "texts do"} not give it: add it in the Texts panel's Details or with Import details….`;
}

/** Tab-separated, a row per value and the totals. */
export function compositionTsv(rows: readonly Share[], key: string): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  const sum = (f: (r: Share) => number) => rows.reduce((n, r) => n + f(r), 0);
  return [
    [keyLabel(key), "texts", "sentences", "words", "glossed words"].join("\t"),
    ...rows.map((r) => [clean(r.value), r.texts, r.sentences, r.words, r.glossed].join("\t")),
    ["All texts", sum((r) => r.texts), sum((r) => r.sentences), sum((r) => r.words), sum((r) => r.glossed)].join("\t"),
  ].join("\n");
}
