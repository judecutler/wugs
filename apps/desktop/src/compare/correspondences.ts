// Sound correspondences: the project's words and another language's words
// for the same concepts aligned sound by sound, and the pairs of sounds
// counted, so recurring correspondences (p : f in many words) stand out
// from chance resemblances, the first step of the comparative method.
// Pure; npm-tested.

import { sounds } from "../sounds/ipa.ts";
import { classOf } from "../sounds/shapes.ts";
import { similarity, type Row } from "./logic.ts";

/** A sound with nothing across from it in the other word. */
export const GAP = "–";

/** One step of an alignment: the project's sound and theirs (GAP for none). */
export type Slot = [ours: string, theirs: string];

/** Substitution cost: the same sound nothing, a vowel for a vowel or a
 * consonant for a consonant less than across the two, so vowels line up
 * with vowels. */
function cost(a: string, b: string): number {
  if (a === b) return 0;
  const [x, y] = [classOf(a), classOf(b)];
  return x === y && x !== "?" ? 0.6 : 1.2;
}

/** Two forms aligned sound by sound by least edit cost (an insertion or
 * deletion costs 1); affix markers are left out. */
export function align(ours: string, theirs: string): Slot[] {
  const x = sounds(ours.replace(/[-=]/gu, ""));
  const y = sounds(theirs.replace(/[-=]/gu, ""));
  const d: number[][] = Array.from({ length: x.length + 1 }, (_, i) => Array.from({ length: y.length + 1 }, (_, j) => (i === 0 ? j : j === 0 ? i : 0)));
  for (let i = 1; i <= x.length; i++)
    for (let j = 1; j <= y.length; j++) d[i]![j] = Math.min(d[i - 1]![j]! + 1, d[i]![j - 1]! + 1, d[i - 1]![j - 1]! + cost(x[i - 1]!, y[j - 1]!));
  const out: Slot[] = [];
  let [i, j] = [x.length, y.length];
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && Math.abs(d[i]![j]! - (d[i - 1]![j - 1]! + cost(x[i - 1]!, y[j - 1]!))) < 1e-9) {
      out.push([x[i - 1]!, y[j - 1]!]);
      i--;
      j--;
    } else if (i > 0 && Math.abs(d[i]![j]! - (d[i - 1]![j]! + 1)) < 1e-9) {
      out.push([x[i - 1]!, GAP]);
      i--;
    } else {
      out.push([GAP, y[j - 1]!]);
      j--;
    }
  }
  return out.reverse();
}

export type Correspondence = {
  ours: string;
  theirs: string;
  count: number;
  /** The concepts it was met in, in list order, at most `EXAMPLES`. */
  concepts: string[];
  same: boolean;
};

export type Correspondences = { pairs: number; list: Correspondence[] };

const EXAMPLES = 4;

/** For one language, each concept's closest pair of words at least `min`
 * alike, aligned; every pair of sounds counted. Sorted by the project's
 * sound in order of first use, then by count. */
export function correspondences(rows: readonly Row[], language: string, min: number): Correspondences {
  const tally = new Map<string, Correspondence>();
  const order = new Map<string, number>();
  let pairs = 0;
  for (const r of rows) {
    let best: { ours: string; theirs: string; s: number } | null = null;
    for (const c of r.others.get(language) ?? []) {
      if (c.similarity === null || c.similarity < min) continue;
      for (const o of r.ours) {
        const s = similarity(o, c.form);
        if (best === null || s > best.s) best = { ours: o, theirs: c.form, s };
      }
    }
    if (best === null) continue;
    pairs += 1;
    const gloss = r.concept.gloss;
    for (const [a, b] of align(best.ours, best.theirs)) {
      if (!order.has(a)) order.set(a, order.size);
      const key = `${a}\u001f${b}`;
      const t = tally.get(key) ?? { ours: a, theirs: b, count: 0, concepts: [], same: a === b };
      t.count += 1;
      if (t.concepts.length < EXAMPLES && !t.concepts.includes(gloss)) t.concepts.push(gloss);
      tally.set(key, t);
    }
  }
  const rank = (s: string) => (s === GAP ? Number.MAX_SAFE_INTEGER : order.get(s)!);
  const list = [...tally.values()].sort((a, b) => rank(a.ours) - rank(b.ours) || b.count - a.count || a.theirs.localeCompare(b.theirs));
  return { pairs, list };
}

/** The line over the table. */
export function correspondenceLine(c: Correspondences, language: string, min: number): string {
  if (c.pairs === 0) return `No word of ${language} is ${Math.round(min * 100)}% alike or more to an answer of yours: try a lower threshold.`;
  const differ = c.list.filter((x) => !x.same && x.count >= 2).length;
  const words = c.pairs === 1 ? "1 pair of words" : `${c.pairs} pairs of words`;
  return `${words} at least ${Math.round(min * 100)}% alike, aligned sound by sound. ${differ} ${differ === 1 ? "correspondence of different sounds recurs" : "correspondences of different sounds recur"}; one met once may be chance or a slip.`;
}

/** The table for a spreadsheet: ours, theirs, count, concepts. */
export function correspondenceTsv(c: Correspondences, language: string): string {
  const rows = c.list.map((x) => [x.ours, x.theirs, String(x.count), x.concepts.join(", ")].join("\t"));
  return [["This project", language, "Count", "Concepts"].join("\t"), ...rows].join("\n") + "\n";
}
