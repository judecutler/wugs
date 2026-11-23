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

export type Cluster = { cluster: string; initial: number; medial: number; final: number; words: number; example: string };

/** Every run of two or more consonants in the words, by where it stands
 * (starting the word, inside it, ending it), counted in running words,
 * commonest first. A word of consonants only is left out; a letter the
 * chart does not place breaks a run. */
export function consonantClusters(words: readonly { word: string; count: number }[]): Cluster[] {
  const by = new Map<string, Cluster>();
  for (const { word, count } of words) {
    const s = sounds(word);
    const cls = s.map(classOf);
    if (!cls.includes("V")) continue;
    const seen = new Set<string>();
    let i = 0;
    while (i < s.length) {
      if (cls[i] !== "C") {
        i += 1;
        continue;
      }
      let j = i;
      while (j < s.length && cls[j] === "C") j += 1;
      if (j - i >= 2) {
        const cluster = s.slice(i, j).join("");
        const r = by.get(cluster) ?? { cluster, initial: 0, medial: 0, final: 0, words: 0, example: word };
        if (i === 0) r.initial += count;
        else if (j === s.length) r.final += count;
        else r.medial += count;
        if (!seen.has(cluster)) {
          r.words += 1;
          seen.add(cluster);
        }
        by.set(cluster, r);
      }
      i = j;
    }
  }
  const total = (c: Cluster) => c.initial + c.medial + c.final;
  return [...by.values()].sort((a, b) => total(b) - total(a) || a.cluster.localeCompare(b.cluster));
}

/** The most consonants a word starts and ends with, and how many clusters there are, in words. */
export function clustersNote(words: readonly { word: string; count: number }[], clusters: readonly Cluster[]): string {
  let onset = 0;
  let coda = 0;
  for (const { word } of words) {
    const cls = sounds(word).map(classOf);
    if (!cls.includes("V")) continue;
    const first = cls.indexOf("V");
    const last = cls.length - 1 - cls.lastIndexOf("V");
    if (!cls.slice(0, first).includes("?")) onset = Math.max(onset, first);
    if (!cls.slice(cls.length - last).includes("?")) coda = Math.max(coda, last);
  }
  const n = (k: number) => (k === 0 ? "no consonant" : k === 1 ? "one consonant" : `up to ${k} consonants`);
  const head = `Words start with ${n(onset)} before the first vowel and end with ${n(coda)} after the last.`;
  if (clusters.length === 0) return `${head} No two consonants stand together.`;
  return `${head} ${clusters.length} ${clusters.length === 1 ? "cluster" : "clusters"} of two or more consonants, commonest first.`;
}
