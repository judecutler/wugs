// Minimal pairs: words that differ in exactly one sound, the evidence a
// phonologist looks for that two sounds contrast (SIL's Phonology Assistant
// lists them the same way). Pure and npm-tested; words are cut into sounds
// by ./ipa.ts, so `tʰa` and `ta` are a pair for tʰ and t, and `pata` and
// `bata` one for p and b.
import { sounds } from "./ipa.ts";

export type Pair = { wordA: string; wordB: string; countA: number; countB: number };

export type Contrast = {
  /** The two sounds, in code-point order. */
  a: string;
  b: string;
  /** Pairs found, at most `PAIRS_KEPT`, the commonest words first. */
  pairs: Pair[];
  /** Pairs there are in all. */
  total: number;
};

/** The commonest word types read, so a large corpus stays quick. */
export const WORDS_READ = 20000;
export const PAIRS_KEPT = 60;

const SEP = "\u001F";
const HOLE = "\u0000";

/** Every contrast the words show, the best attested first. */
export function minimalPairs(words: readonly { word: string; count: number }[], limit = WORDS_READ): Contrast[] {
  const read = [...words].sort((x, y) => y.count - x.count).slice(0, limit);
  // Words with one sound left out, by what is left: two words in one group
  // with different sounds in the hole differ only there.
  const groups = new Map<string, { word: string; sound: string; count: number }[]>();
  const seenWord = new Set<string>();
  for (const { word, count } of read) {
    if (seenWord.has(word)) continue;
    seenWord.add(word);
    const s = sounds(word);
    if (s.length === 0) continue;
    for (let i = 0; i < s.length; i++) {
      const key = [...s.slice(0, i), HOLE, ...s.slice(i + 1)].join(SEP);
      const list = groups.get(key) ?? [];
      list.push({ word, sound: s[i] ?? "", count });
      groups.set(key, list);
    }
  }
  const contrasts = new Map<string, Contrast>();
  for (const list of groups.values()) {
    if (list.length < 2) continue;
    for (let i = 0; i < list.length; i++) {
      for (let j = i + 1; j < list.length; j++) {
        const x = list[i];
        const y = list[j];
        if (x === undefined || y === undefined || x.sound === y.sound || x.word === y.word) continue;
        const [first, second] = x.sound < y.sound ? [x, y] : [y, x];
        const key = `${first.sound}${SEP}${second.sound}`;
        const c = contrasts.get(key) ?? { a: first.sound, b: second.sound, pairs: [], total: 0 };
        contrasts.set(key, c);
        c.total += 1;
        c.pairs.push({ wordA: first.word, wordB: second.word, countA: first.count, countB: second.count });
      }
    }
  }
  const out = [...contrasts.values()];
  for (const c of out) {
    c.pairs.sort((p, q) => Math.min(q.countA, q.countB) - Math.min(p.countA, p.countB) || p.wordA.localeCompare(q.wordA));
    c.pairs = c.pairs.slice(0, PAIRS_KEPT);
  }
  return out.sort((p, q) => q.total - p.total || p.a.localeCompare(q.a) || p.b.localeCompare(q.b));
}

/** The contrasts that involve one sound, or every contrast when it is null. */
export function contrastsOf(all: readonly Contrast[], sound: string | null): Contrast[] {
  return sound === null ? [...all] : all.filter((c) => c.a === sound || c.b === sound);
}

/** A contrast's label, e.g. `p ~ b (3 pairs)`. */
export function contrastLabel(c: Contrast): string {
  return `${c.a} ~ ${c.b} (${c.total} ${c.total === 1 ? "pair" : "pairs"})`;
}

/** The line over the list, in words. */
export function pairsNote(all: readonly Contrast[], readWords: number, totalWords: number): string {
  if (all.length === 0) return "No two words here differ in just one sound yet.";
  const read = readWords < totalWords ? ` among the ${readWords.toLocaleString()} commonest of ${totalWords.toLocaleString()} word types` : "";
  return `${all.length} ${all.length === 1 ? "pair of sounds is" : "pairs of sounds are"} told apart by at least one minimal pair${read}.`;
}

/** The pairs as a table for a spreadsheet. */
export function pairsTable(c: Contrast): string {
  return `${[`${c.a}\t${c.b}`, ...c.pairs.map((p) => `${p.wordA}\t${p.wordB}`)].join("\n")}\n`;
}
