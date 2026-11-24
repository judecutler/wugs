// Syllable types: each word cut into syllables by the maximal onset
// principle, with the consonant sequences that start words as the onsets
// the language allows, and the syllable shapes counted (CV, CVC, V, …), the
// start of a grammar's syllable-structure section. Each vowel is a nucleus,
// so a diphthong written as two vowels counts as two syllables. Read from
// the word types the Sounds panel reads; pure, npm-tested.

import { sounds } from "./ipa.ts";
import { classOf } from "./shapes.ts";

export type SyllableType = { shape: string; count: number; example: string };

/** The consonant sequences words start with (as sounds joined by U+0000), the empty one included. */
export function legalOnsets(words: readonly { word: string }[]): Set<string> {
  const out = new Set<string>([""]);
  for (const { word } of words) {
    const s = sounds(word);
    const cls = s.map(classOf);
    const first = cls.indexOf("V");
    if (first < 0 || cls.slice(0, first).includes("?")) continue;
    out.add(s.slice(0, first).join("\u0000"));
  }
  return out;
}

/** A word cut into syllables (each a list of sounds), or null when it has
 * no vowel or a letter the chart does not place. Between two vowels the
 * next syllable takes the longest run of consonants that can start a word,
 * and always a single consonant. */
export function syllabify(word: string, onsets: ReadonlySet<string>): string[][] | null {
  const s = sounds(word);
  const cls = s.map(classOf);
  if (!cls.includes("V") || cls.includes("?")) return null;
  const nuclei = cls.flatMap((c, i) => (c === "V" ? [i] : []));
  const out: string[][] = [];
  let start = 0;
  for (let k = 0; k < nuclei.length; k++) {
    const v = nuclei[k]!;
    const next = nuclei[k + 1];
    if (next === undefined) {
      out.push(s.slice(start));
      break;
    }
    // Consonants between this vowel and the next: the next syllable's onset
    // is the longest tail of them that can start a word (one consonant always can).
    let cut = v + 1;
    while (cut < next - 1 && !onsets.has(s.slice(cut, next).join("\u0000"))) cut += 1;
    out.push(s.slice(start, cut));
    start = cut;
  }
  return out;
}

const shapeOf = (syllable: readonly string[]): string => syllable.map(classOf).join("");

/** Every syllable shape in running words, commonest first, with an example
 * word written with dots between its syllables. */
export function syllableTypes(words: readonly { word: string; count: number }[]): SyllableType[] {
  const onsets = legalOnsets(words);
  const by = new Map<string, SyllableType>();
  for (const { word, count } of words) {
    const cut = syllabify(word, onsets);
    if (cut === null) continue;
    const example = cut.map((x) => x.join("")).join(".");
    for (const syl of cut) {
      const shape = shapeOf(syl);
      const t = by.get(shape) ?? { shape, count: 0, example };
      t.count += count;
      by.set(shape, t);
    }
  }
  return [...by.values()].sort((a, b) => b.count - a.count || a.shape.localeCompare(b.shape));
}

/** The syllable template and the share of open syllables, in words:
 * "(C)(C)V(C): up to 2 consonants before the vowel and 1 after; 78% of syllables are open." */
export function syllableNote(types: readonly SyllableType[]): string {
  if (types.length === 0) return "No word with a vowel to cut into syllables.";
  const onset = (t: SyllableType) => t.shape.indexOf("V");
  const coda = (t: SyllableType) => t.shape.length - 1 - t.shape.lastIndexOf("V");
  const [minOn, maxOn] = [Math.min(...types.map(onset)), Math.max(...types.map(onset))];
  const [minCo, maxCo] = [Math.min(...types.map(coda)), Math.max(...types.map(coda))];
  const part = (min: number, max: number) => "C".repeat(min) + "(C)".repeat(max - min);
  const template = `${part(minOn, maxOn)}V${part(minCo, maxCo)}`;
  const total = types.reduce((n, t) => n + t.count, 0);
  const open = types.filter((t) => coda(t) === 0).reduce((n, t) => n + t.count, 0);
  const n = (x: number, what: string) => `${x} ${what}${x === 1 ? "" : "s"}`;
  const before = minOn === maxOn ? `${minOn === 0 ? "no consonant" : n(minOn, "consonant")} before the vowel` : `${minOn === 0 ? "up to" : `${minOn} to`} ${n(maxOn, "consonant")} before the vowel`;
  const after = maxCo === 0 ? "none after" : minCo === maxCo ? `${maxCo} after` : `up to ${maxCo} after`;
  return `${template}: ${before} and ${after}; ${Math.round((100 * open) / total)}% of syllables are open (end in a vowel). Each vowel counts as a syllable, so a diphthong written as two vowels is two.`;
}
