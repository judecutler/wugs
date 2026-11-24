// The Interlinear panel's rules (pure, so node --test runs them): which
// segments of a text are its sentences, and how far its glossing has come.

import type { ExampleWord } from "../gloss/example";

export type Seg = { id: string; tier: string; parent: string | null; startMs: number | null; content: string };
export type Tier = { id: string; utterance: boolean; role?: string | null; name?: string };

/** The tracks a text's sentences are on: the utterance track when this text
 * uses it; else every track with sentences that is not a translation, as a
 * transcript with a track per speaker (CHAT) has. */
export function sentenceTracks(segments: readonly Seg[], tiers: readonly Tier[]): string[] {
  const used = [...new Set(segments.filter((s) => s.parent === null).map((s) => s.tier))].filter((id) => tiers.find((t) => t.id === id)?.role !== "translation");
  const utterance = tiers.find((t) => t.utterance)?.id;
  return utterance !== undefined && used.includes(utterance) ? [utterance] : used;
}

/** The text's sentences on its sentence tracks, translations left out, in numbered order. */
export function sentencesOf<S extends Seg>(segments: readonly S[], tiers: readonly Tier[], numbers: Map<string, number>): S[] {
  const tracks = sentenceTracks(segments, tiers);
  return segments.filter((s) => tracks.includes(s.tier) && s.parent === null).sort((a, b) => (numbers.get(a.id) ?? 0) - (numbers.get(b.id) ?? 0));
}

/** One sentence as the panel draws it. */
export type Line = { id: string; number: number; content: string; words: ExampleWord[]; translation: string | null; startMs: number | null; notes?: string[]; speaker?: string };

export const analysed = (l: Line): boolean => l.words.some((w) => w.morphs.length > 0);

/** The line over the text: how many sentences are glossed, and how many words. */
export function progress(lines: readonly Line[]): string {
  if (lines.length === 0) return "This text has no sentences yet.";
  // A sentence counts as glossed once no word is left to gloss.
  const done = lines.filter((l) => l.words.length > 0 && l.words.every((w) => w.morphs.length > 0)).length;
  const words = lines.reduce((n, l) => n + l.words.length, 0);
  const glossedWords = lines.reduce((n, l) => n + l.words.filter((w) => w.morphs.length > 0).length, 0);
  const pct = words === 0 ? 0 : Math.round((glossedWords / words) * 100);
  return `${done} of ${lines.length} ${lines.length === 1 ? "sentence" : "sentences"} glossed; ${glossedWords} of ${words} ${words === 1 ? "word" : "words"} (${pct}%).`;
}

/** A sentence that still needs glossing: transcribed, and not yet broken
 * into words or with a word that has no analysis. */
export const toGloss = (l: Line): boolean => l.content.trim() !== "" && (l.words.length === 0 || l.words.some((w) => w.morphs.length === 0));

/** A transcribed sentence with no translation yet. */
export const toTranslate = (l: Line): boolean => l.content.trim() !== "" && (l.translation === null || l.translation.trim() === "");

/** Which sentences the panel lists. */
export type Show = "all" | "gloss" | "translate";

export const SHOWS: { id: Show; label: string; next: string; done: string; keep: (l: Line) => boolean }[] = [
  { id: "all", label: "Every sentence", next: "Next to gloss", done: "Every sentence of this text is glossed.", keep: () => true },
  { id: "gloss", label: "Sentences still to gloss", next: "Next to gloss", done: "Every sentence of this text is glossed.", keep: toGloss },
  { id: "translate", label: "Sentences without a translation", next: "Next to translate", done: "Every sentence of this text has a translation.", keep: toTranslate },
];

export const showOf = (id: string | null): (typeof SHOWS)[number] => SHOWS.find((s) => s.id === id) ?? SHOWS[0]!;

/** The next sentence after `current` that `wanted` keeps (from the start when
 * nothing or a sentence not listed is current), wrapping round; null when
 * none is left. */
export function nextWhere(lines: readonly Line[], current: string | null, wanted: (l: Line) => boolean): Line | null {
  const at = lines.findIndex((l) => l.id === current);
  for (let k = 1; k <= lines.length; k++) {
    const l = lines[(at + k + lines.length) % lines.length]!;
    if (wanted(l)) return l;
  }
  return null;
}

/** The next sentence still to gloss after `current`. */
export const nextToGloss = (lines: readonly Line[], current: string | null): Line | null => nextWhere(lines, current, toGloss);
