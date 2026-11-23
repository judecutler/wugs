// Where sounds occur and what shape words take: the distribution and
// phonotactics tables a grammar's phonology chapter starts from. Each sound
// is counted at the start, inside and at the end of words, so gaps (no voiced
// stop word-finally, no velar nasal word-initially) stand out; each word is
// read as its consonant–vowel skeleton (CVCV, CVC). Pure and npm-tested;
// sounds and their classes come from ./ipa.ts.
import { placeOf, sounds } from "./ipa.ts";

export type Where = { sound: string; initial: number; medial: number; final: number; alone: number };

/** C for a consonant, V for a vowel, ? for a letter the chart does not place. */
export function classOf(sound: string): "C" | "V" | "?" {
  const p = placeOf(sound);
  if (p.kind === "vowel") return "V";
  if (p.kind === "unplaced") return "?";
  return "C";
}

/** Running counts of each sound at the start, inside and at the end of
 * words; a one-sound word counts as `alone`. In the order `order` gives. */
export function positions(words: readonly { word: string; count: number }[], order: readonly string[]): Where[] {
  const by = new Map<string, Where>();
  const row = (s: string): Where => {
    let r = by.get(s);
    if (r === undefined) {
      r = { sound: s, initial: 0, medial: 0, final: 0, alone: 0 };
      by.set(s, r);
    }
    return r;
  };
  for (const { word, count } of words) {
    const s = sounds(word);
    s.forEach((x, i) => {
      const r = row(x);
      if (s.length === 1) r.alone += count;
      else if (i === 0) r.initial += count;
      else if (i === s.length - 1) r.final += count;
      else r.medial += count;
    });
  }
  const rank = new Map(order.map((s, i) => [s, i]));
  return [...by.values()].sort((a, b) => (rank.get(a.sound) ?? 1e9) - (rank.get(b.sound) ?? 1e9) || a.sound.localeCompare(b.sound));
}

export type Shape = { shape: string; count: number; words: number; example: string };

/** Each word's consonant–vowel skeleton with how often it occurs, commonest first. */
export function shapes(words: readonly { word: string; count: number }[]): Shape[] {
  const by = new Map<string, Shape>();
  for (const { word, count } of words) {
    const skeleton = sounds(word).map(classOf).join("");
    if (skeleton === "") continue;
    const r = by.get(skeleton) ?? { shape: skeleton, count: 0, words: 0, example: word };
    r.count += count;
    r.words += 1;
    by.set(skeleton, r);
  }
  return [...by.values()].sort((a, b) => b.count - a.count || a.shape.localeCompare(b.shape));
}

/** A share of a total as a whole percent, or "<1%" for a small one. */
export function share(n: number, total: number): string {
  if (total === 0 || n === 0) return "0%";
  const p = (100 * n) / total;
  return p < 1 ? "<1%" : `${Math.round(p)}%`;
}

/** Sounds never found in a position where most sounds of their class are, in words. */
export function gapsNote(rows: readonly Where[]): string {
  const missing = (key: "initial" | "final") => rows.filter((r) => classOf(r.sound) === "C" && r[key] === 0 && r.initial + r.medial + r.final >= 3).map((r) => r.sound);
  const parts: string[] = [];
  const start = missing("initial");
  const end = missing("final");
  if (start.length > 0) parts.push(`never at the start of a word: ${start.join(" ")}`);
  if (end.length > 0) parts.push(`never at the end: ${end.join(" ")}`);
  return parts.length === 0 ? "" : `Consonants used at least three times but ${parts.join("; ")}.`;
}
