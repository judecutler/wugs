// Vowel harmony: within a word, do neighbouring vowels agree in backness,
// rounding or height more often than chance? Each word type's vowels are
// read in order and every vowel paired with the next; a pair agrees on a
// feature when both vowels fall in the same class of it. Chance is what the
// classes' shares among first and second vowels give if they were
// independent; the strength is Cohen's kappa, (observed − chance) /
// (1 − chance), 0 for none and 1 for every pair agreeing. Pure and npm-tested.
import { placeOf, sounds, type VowelPlace } from "./ipa.ts";

export type Word = { word: string; count: number };

export type Feature = "back" | "round" | "height";

const FEATURES: { id: Feature; name: string; classes: string[]; of: (v: VowelPlace) => number | null }[] = [
  // Central vowels are left out of backness: they are often neutral in harmony
  // systems. So is plain a: the chart puts it front, but most transcriptions
  // write it for an open central or back vowel (Turkish a is back); æ and ɑ count.
  { id: "back", name: "backness", classes: ["front", "back"], of: (v) => (v.height === 6 && v.back === 0 && !v.rounded ? null : v.back <= 0.5 ? 0 : v.back >= 1.5 ? 1 : null) },
  { id: "round", name: "rounding", classes: ["unrounded", "rounded"], of: (v) => (v.rounded ? 1 : 0) },
  { id: "height", name: "height", classes: ["close", "mid", "open"], of: (v) => (v.height <= 1 ? 0 : v.height <= 4 ? 1 : 2) },
];

export type Agreement = { feature: Feature; name: string; pairs: number; observed: number; expected: number; kappa: number };

/** Fewer vowel pairs than this, and nothing is said about a feature. */
export const MIN_PAIRS = 30;

function vowelsOf(word: string): VowelPlace[] {
  const out: VowelPlace[] = [];
  for (const s of sounds(word.normalize("NFC").toLocaleLowerCase())) {
    const p = placeOf(s);
    if (p.kind === "vowel") out.push(p.at);
  }
  return out;
}

/** Each feature's agreement over every pair of neighbouring vowels in the word types. */
export function harmony(words: readonly Word[]): Agreement[] {
  const seen = new Set<string>();
  const cut: VowelPlace[][] = [];
  for (const { word } of words) {
    const k = word.normalize("NFC").toLocaleLowerCase();
    if (seen.has(k)) continue;
    seen.add(k);
    const vs = vowelsOf(k);
    if (vs.length >= 2) cut.push(vs);
  }
  return FEATURES.map((f) => {
    const first = new Array<number>(f.classes.length).fill(0);
    const second = new Array<number>(f.classes.length).fill(0);
    let pairs = 0;
    let agree = 0;
    for (const vs of cut) {
      for (let i = 1; i < vs.length; i++) {
        const a = f.of(vs[i - 1]!);
        const b = f.of(vs[i]!);
        if (a === null || b === null) continue;
        pairs += 1;
        first[a]! += 1;
        second[b]! += 1;
        if (a === b) agree += 1;
      }
    }
    const observed = pairs === 0 ? 0 : agree / pairs;
    const expected = pairs === 0 ? 0 : first.reduce((s, n, c) => s + (n / pairs) * (second[c]! / pairs), 0);
    const kappa = expected >= 1 ? 0 : (observed - expected) / (1 - expected);
    return { feature: f.id, name: f.name, pairs, observed, expected, kappa };
  });
}

/** How strong a kappa is, in words. */
export function strength(kappa: number): "strong" | "a tendency" | "none" {
  return kappa >= 0.5 ? "strong" : kappa >= 0.2 ? "a tendency" : "none";
}

const pct = (x: number) => `${Math.round(x * 100)}%`;

/** One line per feature with enough pairs. */
export function harmonyLines(all: readonly Agreement[]): string[] {
  const told = all.filter((a) => a.pairs >= MIN_PAIRS);
  if (told.length === 0) return all.some((a) => a.pairs > 0) ? [`Too few words with two vowels or more yet (${MIN_PAIRS} pairs of neighbouring vowels are needed).`] : [];
  const lines = told.map((a) => {
    const s = strength(a.kappa);
    const verdict = s === "strong" ? "harmony in this feature is likely" : s === "a tendency" ? "a tendency to agree" : a.kappa <= -0.2 ? "they tend to differ (a dissimilation)" : "no more than chance";
    return `${a.name[0]!.toUpperCase()}${a.name.slice(1)}: ${pct(a.observed)} of ${a.pairs.toLocaleString("en")} neighbouring vowels agree, against ${pct(a.expected)} by chance (κ = ${a.kappa.toFixed(2)}): ${verdict}.`;
  });
  lines.push("Counted over word types, each vowel with the next in the word. Central vowels and plain a are left out of backness: harmony systems often treat central vowels as neutral, and a is written for front, central and back vowels alike.");
  return lines;
}
