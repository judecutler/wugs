// How close a speaker's forms come to their targets: PhonBank's %mod
// (target) against %pho (actual), scored as Shriberg and Kwiatkowski's
// percent consonants correct and its kin. Each word of the target is
// aligned with the same word of the actual form (or the whole utterance
// when the word counts differ) by the least-cost alignment the word-list
// comparison uses. Pure.
import { align, GAP } from "../compare/correspondences.ts";
import { classOf } from "./shapes.ts";

export type PhonPair = { track: string; text: string; title: string; segment: string; target: string; actual: string };
export type PhonPairs = { pairs: PhonPair[]; tracks: [string, string][]; omitted: number };

export type Score = { utterances: number; consonants: number; consonantsRight: number; vowels: number; vowelsRight: number; words: number; wordsRight: number };

const empty = (): Score => ({ utterances: 0, consonants: 0, consonantsRight: 0, vowels: 0, vowelsRight: 0, words: 0, wordsRight: 0 });

/** The words of a %mod or %pho line, punctuation and CHAT's terminators left out. */
const wordsOf = (line: string): string[] => line.split(/\s+/u).map((w) => w.replace(/^[.,!?;:"“”]+|[.,!?;:"“”]+$/gu, "")).filter((w) => w !== "" && w !== "+..." && w !== "xxx");

/** Target and actual word by word, or the whole lines as one when the word counts differ. */
export function wordPairs(target: string, actual: string): [string, string][] {
  const t = wordsOf(target);
  const a = wordsOf(actual);
  if (t.length === a.length) return t.map((w, i) => [w, a[i]!]);
  return t.length === 0 ? [] : [[t.join(""), a.join("")]];
}

/** One utterance's tally, added to `into`; each substitution and deletion counted in `changes` (`t→a`, `∅` for nothing). */
export function score(target: string, actual: string, into: Score = empty(), changes?: Map<string, number>): Score {
  into.utterances += 1;
  for (const [t, a] of wordPairs(target, actual)) {
    let right = true;
    for (const [x, y] of align(t, a)) {
      if (x !== y) right = false;
      if (x === GAP) continue;
      const c = classOf(x);
      if (c === "C") {
        into.consonants += 1;
        if (x === y) into.consonantsRight += 1;
      } else if (c === "V") {
        into.vowels += 1;
        if (x === y) into.vowelsRight += 1;
      }
      if (x !== y && changes !== undefined) {
        const k = `${x}→${y === GAP ? "∅" : y}`;
        changes.set(k, (changes.get(k) ?? 0) + 1);
      }
    }
    into.words += 1;
    if (right) into.wordsRight += 1;
  }
  return into;
}

export const percent = (right: number, all: number): string => (all === 0 ? "–" : `${((100 * right) / all).toFixed(1)}%`);

/** Percent consonants correct, percent vowels correct, percent phonemes correct, whole words right. */
export const scoreCells = (s: Score): string[] => [percent(s.consonantsRight, s.consonants), percent(s.vowelsRight, s.vowels), percent(s.consonantsRight + s.vowelsRight, s.consonants + s.vowels), percent(s.wordsRight, s.words)];

export type AccuracyRow = { track: string; name: string; text: string; title: string; score: Score };

/** A row per speaker and session (text), sessions in the order met, then a total per speaker; and each speaker's changes, commonest first. */
export function accuracy(data: PhonPairs): { rows: AccuracyRow[]; totals: AccuracyRow[]; changes: Map<string, [string, number][]> } {
  const names = new Map(data.tracks);
  const rows = new Map<string, AccuracyRow>();
  const totals = new Map<string, AccuracyRow>();
  const changes = new Map<string, Map<string, number>>();
  for (const p of data.pairs) {
    const k = `${p.track}\u0000${p.text}`;
    if (!rows.has(k)) rows.set(k, { track: p.track, name: names.get(p.track) ?? "", text: p.text, title: p.title, score: empty() });
    if (!totals.has(p.track)) totals.set(p.track, { track: p.track, name: names.get(p.track) ?? "", text: "", title: "All sessions", score: empty() });
    if (!changes.has(p.track)) changes.set(p.track, new Map());
    const ch = changes.get(p.track)!;
    score(p.target, p.actual, rows.get(k)!.score, ch);
    score(p.target, p.actual, totals.get(p.track)!.score);
  }
  const sorted = new Map([...changes].map(([t, m]) => [t, [...m].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))]));
  return { rows: [...rows.values()], totals: [...totals.values()], changes: sorted };
}

export function accuracyNote(totals: readonly AccuracyRow[]): string {
  if (totals.length === 0) return "";
  const who = totals.map((t) => `${t.name || "(unnamed)"} ${percent(t.score.consonantsRight, t.score.consonants)}`).join(", ");
  return `Percent consonants correct (PCC, Shriberg and Kwiatkowski 1982) over every session: ${who}. Each target word (%mod) is lined up with what was said (%pho) sound by sound; a sound left out counts as wrong, an added one is not counted. Under about 85% a child's speech is usually judged to have errors for their age; read it against their age and language.`;
}

export const ACCURACY_COLUMNS = ["Speaker", "Session", "Utterances", "PCC", "PVC", "PPC", "Words right"];

export function accuracyTsv(rows: readonly AccuracyRow[]): string {
  return [ACCURACY_COLUMNS.join("\t"), ...rows.map((r) => [r.name, r.title, String(r.score.utterances), ...scoreCells(r.score)].join("\t"))].join("\n") + "\n";
}
