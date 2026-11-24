// Rare sound sequences: two sounds side by side (or a sound at a word's edge)
// that only one rare word in the project has, though each sound is common.
// Such a word is often a slip of the ear or keyboard, a loanword, or a real
// gap in what the transcriptions show; worth a look either way. Pure; no DOM.
import { sounds } from "../sounds/ipa.ts";

export type Word = { word: string; count: number };
export type Unusual = { word: string; count: number; sequences: string[]; score: number };

/** A word's edge, in a sequence. */
export const EDGE = "#";
/** Below this many running words, every sequence is rare. */
export const MIN_RUNNING = 500;
/** A word used more often than this is no slip. */
export const MAX_USES = 2;
/** A sound used fewer times than this is rare itself, so its sequences say little. */
export const MIN_SOUND = 10;
export const MAX_LISTED = 200;

const label = (a: string, b: string): string => (a === EDGE ? `#${b}` : b === EDGE ? `${a}#` : `${a}${b}`);

/** The rare words with sequences no other word has, those whose sounds are
 * commonest first. Case is folded; words are read as the Sounds panel cuts them. */
export function unusualWords(words: readonly Word[]): { words: Unusual[]; running: number } {
  const running = words.reduce((n, w) => n + w.count, 0);
  if (running < MIN_RUNNING) return { words: [], running };
  const cut = new Map<string, { count: number; units: string[] }>();
  for (const w of words) {
    const key = w.word.normalize("NFC").toLowerCase();
    const s = sounds(key);
    if (s.length === 0) continue;
    const had = cut.get(key);
    if (had !== undefined) had.count += w.count;
    else cut.set(key, { count: w.count, units: [EDGE, ...s, EDGE] });
  }
  const soundUses = new Map<string, number>();
  const holders = new Map<string, Set<string>>();
  for (const [word, { count, units }] of cut) {
    for (let i = 1; i < units.length - 1; i++) soundUses.set(units[i]!, (soundUses.get(units[i]!) ?? 0) + count);
    for (let i = 0; i + 1 < units.length; i++) {
      const k = `${units[i]}\u0001${units[i + 1]}`;
      const set = holders.get(k) ?? new Set<string>();
      if (set.size < 2) set.add(word);
      holders.set(k, set);
    }
  }
  const uses = (s: string) => (s === EDGE ? Infinity : (soundUses.get(s) ?? 0));
  const out: Unusual[] = [];
  for (const [word, { count, units }] of cut) {
    if (count > MAX_USES) continue;
    const found: { seq: string; score: number }[] = [];
    for (let i = 0; i + 1 < units.length; i++) {
      const [a, b] = [units[i]!, units[i + 1]!];
      if (holders.get(`${a}\u0001${b}`)!.size !== 1) continue;
      const score = Math.min(uses(a), uses(b));
      if (score < MIN_SOUND) continue;
      const seq = label(a, b);
      if (!found.some((f) => f.seq === seq)) found.push({ seq, score });
    }
    if (found.length > 0) out.push({ word, count, sequences: found.map((f) => f.seq), score: Math.max(...found.map((f) => f.score)) });
  }
  out.sort((x, y) => y.score - x.score || y.sequences.length - x.sequences.length || x.word.localeCompare(y.word));
  return { words: out.slice(0, MAX_LISTED), running };
}

/** The sequences in words: `#ŋ` is "ŋ at the start of a word". */
export function sequenceWords(seq: string): string {
  if (seq.startsWith(EDGE)) return `${seq.slice(1)} at the start of a word`;
  if (seq.endsWith(EDGE)) return `${seq.slice(0, -1)} at the end of a word`;
  return seq;
}

/** The section's line. */
export function unusualLine(r: { words: Unusual[]; running: number }): string {
  if (r.running < MIN_RUNNING) return `Too few transcribed words to tell (${r.running.toLocaleString("en")} of at least ${MIN_RUNNING.toLocaleString("en")}).`;
  if (r.words.length === 0) return "Every rare word's sounds come in sequences other words have too.";
  const n = r.words.length;
  return `${n} rare ${n === 1 ? "word has" : "words have"} a sequence of common sounds that no other word has: a slip, a loanword, or a real rarity. Open each to check.`;
}
