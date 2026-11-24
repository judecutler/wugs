// Good dictionary examples first: a score per concordance line in the spirit
// of GDEX (Kilgarriff et al. 2008), Sketch Engine's ranking of example
// sentences for lexicographers. A good example is a whole sentence of a
// readable length, made of common words, with the word in the middle rather
// than at an edge, free of numbers and stray symbols, and (when it is known)
// translated. Each criterion is a factor from 0 to 1; the score is their
// product, so one bad trait costs a line without hiding it. Pure; no DOM.
import type { Hit } from "./logic.ts";

/** Lengths in words inside this range score best. */
export const BEST_SHORTEST = 5;
export const BEST_LONGEST = 15;
/** A word used fewer times than this in the project counts as rare. */
export const RARE = 3;

/** How a word is looked up in the frequency list: NFC, lower case. */
export const freqKey = (w: string): string => w.normalize("NFC").toLowerCase();

const letters = /\p{L}/u;
const oddChar = /[\p{N}\p{S}]|[([{<>}\])@#_/\\|]/u;

export type Gdex = { score: number; why: string[] };

/**
 * A line's score and, in words, what lowered it. `uses` gives a word's count
 * in the project (undefined: not among the counted words, so rare);
 * `translated` is null when whether the line is translated is not known.
 */
export function gdex(hit: Hit, uses: (key: string) => number | undefined, translated: boolean | null): Gdex {
  const why: string[] = [];
  const words = hit.words.filter((w) => letters.test(w));
  const n = words.length;
  let score = 1;
  if (n < BEST_SHORTEST) {
    score *= Math.max(0.2, n / BEST_SHORTEST);
    why.push(`short (${n} ${n === 1 ? "word" : "words"})`);
  } else if (n > BEST_LONGEST) {
    score *= Math.max(0.2, 1 - (n - BEST_LONGEST) / 20);
    why.push(`long (${n} words)`);
  }
  const len = Math.max(1, hit.len ?? 1);
  const context = hit.words.filter((_, i) => i < hit.at || i >= hit.at + len).filter((w) => letters.test(w));
  const rare = context.filter((w) => (uses(freqKey(w)) ?? 0) < RARE);
  if (rare.length > 0 && context.length > 0) {
    score *= 1 - 0.8 * (rare.length / context.length);
    why.push(`${rare.length} rare ${rare.length === 1 ? "word" : "words"} (${rare.slice(0, 3).join(", ")})`);
  }
  if (hit.at === 0 && hit.words.length > len) {
    // Fine in most languages, but the word mid-sentence shows more of how it is used.
    score *= 0.95;
  }
  if (hit.at + len >= hit.words.length && hit.at > 0) {
    score *= 0.9;
    why.push("ends the sentence");
  }
  const odd = hit.words.filter((w) => oddChar.test(w)).length;
  if (odd > 0) {
    score *= Math.max(0.5, 1 - 0.15 * odd);
    why.push(`${odd} ${odd === 1 ? "number or symbol" : "numbers or symbols"}`);
  }
  if (translated === false) {
    score *= 0.7;
    why.push("not translated");
  }
  return { score, why };
}

/** Lines best first; ties keep the order they came in (text order). */
export function bestFirst(hits: readonly Hit[], uses: (key: string) => number | undefined, translated: (h: Hit) => boolean | null): Hit[] {
  return hits
    .map((h, i) => ({ h, i, s: gdex(h, uses, translated(h)).score }))
    .sort((a, b) => b.s - a.s || a.i - b.i)
    .map((x) => x.h);
}

/** A line's title: its score and what lowered it. */
export function gdexTitle(g: Gdex): string {
  return `As a dictionary example: ${Math.round(g.score * 100)} of 100${g.why.length === 0 ? "" : `; ${g.why.join("; ")}`}.`;
}
