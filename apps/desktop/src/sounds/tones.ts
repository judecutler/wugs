// Tone melodies: each word read as the tones of its vowels (H.L, L.H.H), the
// table a tone chapter starts from (Leben's melodies; which combinations a
// language allows on two-syllable words, and which are missing). Tone is read
// from the marks on vowels (á H, à L, ā M, â HL, ǎ LH, a̋ XH, ȁ XL) or from tone
// letters and tone numbers written after a syllable (ma˥˩, ma⁵¹). Pure and
// npm-tested; vowels come from ./shapes.ts's classOf.
import { sounds } from "./ipa.ts";
import { classOf } from "./shapes.ts";

/** Combining marks read as tones, in NFD. */
export const TONE_MARKS: readonly [string, string, string][] = [
  ["̋", "XH", "a̋"],
  ["́", "H", "á"],
  ["̄", "M", "ā"],
  ["̀", "L", "à"],
  ["̏", "XL", "ȁ"],
  ["̂", "HL", "â"],
  ["̌", "LH", "ǎ"],
];
const MARK = new Map(TONE_MARKS.map(([m, t]) => [m, t]));
/** A vowel with no tone mark: in many spellings the default tone. */
export const UNMARKED = "–";
/** Tone letters and superscript tone numbers. */
const LETTERS = /[˥˦˧˨˩꜒-꜖⁰¹²³⁴⁵]+/gu;
/** Words read; the commonest first. */
export const WORDS_READ = 50_000;
/** Melodies listed for each number of syllables. */
export const PER_LENGTH = 12;

export type Melody = { melody: string; syllables: number; words: number; running: number; example: string };
export type Tones = {
  /** Word types read (with a vowel or a tone letter). */
  types: number;
  /** Word types with at least one tone mark, letter or number. */
  marked: number;
  /** Each tone with the vowels (or syllables) carrying it, in running words. */
  tones: { tone: string; running: number }[];
  /** Melodies by number of syllables, then by word types (at most PER_LENGTH each). */
  melodies: Melody[];
  /** Every two-syllable melody met, listed or not. */
  twoMet: string[];
};

/** One word's melody: each vowel's tone joined by dots, or null with no vowel. */
export function melodyOf(word: string): { melody: string[]; marked: boolean } | null {
  const w = word.normalize("NFC").toLowerCase();
  const runs = w.match(LETTERS);
  if (runs !== null) return { melody: runs, marked: true };
  const tones = sounds(w)
    .filter((s) => classOf(s) === "V")
    .map((s) => {
      const found = [...s.normalize("NFD")].map((c) => MARK.get(c)).filter((t): t is string => t !== undefined);
      return found.length === 0 ? UNMARKED : found.join("");
    });
  if (tones.length === 0) return null;
  return { melody: tones, marked: tones.some((t) => t !== UNMARKED) };
}

/** Every word's melody, counted, for a corpus whose spelling marks tone. */
export function toneMelodies(words: readonly { word: string; count: number }[]): Tones {
  const read = [...words].sort((a, b) => b.count - a.count).slice(0, WORDS_READ);
  const byMelody = new Map<string, Melody>();
  const tones = new Map<string, number>();
  let types = 0;
  let marked = 0;
  for (const { word, count } of read) {
    const m = melodyOf(word);
    if (m === null) continue;
    types += 1;
    if (m.marked) marked += 1;
    for (const t of m.melody) tones.set(t, (tones.get(t) ?? 0) + count);
    const key = m.melody.join(".");
    const row = byMelody.get(key);
    if (row === undefined) byMelody.set(key, { melody: key, syllables: m.melody.length, words: 1, running: count, example: word });
    else {
      row.words += 1;
      row.running += count;
    }
  }
  const all = [...byMelody.values()].sort((a, b) => a.syllables - b.syllables || b.words - a.words || b.running - a.running || (a.melody < b.melody ? -1 : 1));
  const shown = new Map<number, number>();
  const melodies = all.filter((m) => {
    const n = shown.get(m.syllables) ?? 0;
    shown.set(m.syllables, n + 1);
    return n < PER_LENGTH;
  });
  const twoMet = all.filter((m) => m.syllables === 2).map((m) => m.melody);
  return { types, marked, tones: [...tones].map(([tone, running]) => ({ tone, running })).sort((a, b) => b.running - a.running), melodies, twoMet };
}

/** The level tones a corpus uses (one-letter tones and unmarked vowels), for gaps. */
const levels = (t: Tones): string[] => t.tones.map((x) => x.tone).filter((x) => x === UNMARKED || ["XH", "H", "M", "L", "XL"].includes(x));

/** Two-syllable melodies of level tones never met, when two or more levels are in use. */
export function missingPairs(t: Tones): string[] {
  const lv = levels(t);
  if (lv.length < 2 || t.twoMet.length === 0) return [];
  const met = new Set(t.twoMet);
  return lv.flatMap((a) => lv.map((b) => `${a}.${b}`)).filter((m) => !met.has(m));
}

const MARK_EXAMPLE = new Map(TONE_MARKS.map(([, t, e]) => [t, e]));

/** The summary above the table. */
export function tonesNote(t: Tones): string {
  if (t.marked === 0) return "No tone marks in these words (á à ā â ǎ, tone letters such as ˥˩, or tone numbers such as ⁵¹).";
  const share = Math.round((100 * t.marked) / t.types);
  const total = t.tones.reduce((n, x) => n + x.running, 0);
  const list = t.tones
    .slice(0, 8)
    .map((x) => `${x.tone}${MARK_EXAMPLE.has(x.tone) ? ` (${MARK_EXAMPLE.get(x.tone)})` : ""} ${Math.round((100 * x.running) / total)}%`)
    .join(", ");
  const gaps = missingPairs(t);
  return [
    `${t.marked} of ${t.types} word forms (${share}%) carry a tone mark. Tones by vowel: ${list}.`,
    t.tones.some((x) => x.tone === UNMARKED) ? `${UNMARKED} is a vowel with no mark: in many spellings the default tone.` : "",
    gaps.length > 0 ? `Two-syllable melodies never met: ${gaps.join(", ")}.` : "",
  ]
    .filter((s) => s !== "")
    .join(" ");
}

/** The melodies as a table to paste. */
export function tonesTsv(t: Tones): string {
  return ["Syllables\tMelody\tWords\tRunning words\tExample", ...t.melodies.map((m) => [m.syllables, m.melody, m.words, m.running, m.example].join("\t"))].join("\n") + "\n";
}

const STRIP = new RegExp(`[${TONE_MARKS.map(([m]) => m).join("")}]`, "gu");

/** The words with their tone marks, letters and numbers taken off, counts of
 * words that become one merged: so á, à and a count as one vowel in the
 * inventory and the tables built on it. A macron is taken too (read as M). */
export function withoutTone(words: readonly { word: string; count: number }[]): { word: string; count: number }[] {
  const merged = new Map<string, number>();
  for (const { word, count } of words) {
    const bare = word.normalize("NFD").replace(STRIP, "").replace(LETTERS, "").normalize("NFC");
    if (bare === "") continue;
    merged.set(bare, (merged.get(bare) ?? 0) + count);
  }
  return [...merged].map(([word, count]) => ({ word, count })).sort((a, b) => b.count - a.count || (a.word < b.word ? -1 : 1));
}
