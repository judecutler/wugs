// The Interlinear panel's rules (pure, so node --test runs them): which
// segments of a text are its sentences, and how far its glossing has come.

import type { ExampleWord } from "../gloss/example";

export type Seg = { id: string; tier: string; parent: string | null; startMs: number | null; content: string };
export type Tier = { id: string; utterance: boolean };

/** The text's sentences: the utterance track's segments (else the first track used), translations left out, in numbered order. */
export function sentencesOf<S extends Seg>(segments: readonly S[], tiers: readonly Tier[], numbers: Map<string, number>): S[] {
  const track = tiers.find((t) => t.utterance)?.id ?? segments.find((s) => s.parent === null)?.tier ?? "";
  return segments.filter((s) => s.tier === track && s.parent === null).sort((a, b) => (numbers.get(a.id) ?? 0) - (numbers.get(b.id) ?? 0));
}

/** One sentence as the panel draws it. */
export type Line = { id: string; number: number; content: string; words: ExampleWord[]; translation: string | null; startMs: number | null };

export const analysed = (l: Line): boolean => l.words.some((w) => w.morphs.length > 0);

/** The line over the text: how many sentences are glossed, and how many words. */
export function progress(lines: readonly Line[]): string {
  if (lines.length === 0) return "This text has no sentences yet.";
  const done = lines.filter(analysed).length;
  const words = lines.reduce((n, l) => n + l.words.length, 0);
  const glossedWords = lines.reduce((n, l) => n + l.words.filter((w) => w.morphs.length > 0).length, 0);
  const pct = words === 0 ? 0 : Math.round((glossedWords / words) * 100);
  return `${done} of ${lines.length} ${lines.length === 1 ? "sentence" : "sentences"} glossed; ${glossedWords} of ${words} ${words === 1 ? "word" : "words"} (${pct}%).`;
}
