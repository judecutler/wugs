// How typical a word's sounds are in this language, the measures
// psycholinguists choose stimuli and made-up words by (Vitevitch and Luce
// 2004): its neighbours (words one sound added, left out or changed away:
// Luce and Pisoni 1998's neighbourhood density), and its phonotactic
// probability: how often each of its sounds stands in that position in a
// word, and each pair of its sounds next to each other there, counted over
// the word types, each weighted by the log of how often it is used. Pure,
// npm-tested.
import { sounds } from "./ipa.ts";

export type Word = { word: string; count: number };

/** The words read, the commonest first. */
export const WORDS_READ = 20_000;

export type Model = {
  /** Each word type's sounds, by its folded spelling. */
  words: Map<string, { sounds: string[]; count: number; word: string }>;
  /** Sound at position, and the weight of words with a sound at that position. */
  single: Map<string, number>;
  singleTotal: number[];
  /** Two sounds at positions i and i+1, and the weight of words that long. */
  pair: Map<string, number>;
  pairTotal: number[];
  /** Words by their sounds with one marked as a hole (a sound changed), with
   * one left out (a sound added, seen from the shorter word), and as they are. */
  holes: Map<string, string[]>;
  dropped: Map<string, string[]>;
  whole: Map<string, string[]>;
};

const weight = (count: number): number => Math.log10(count + 1);
const fold = (w: string): string => w.normalize("NFC").toLocaleLowerCase();
const SEP = "\u0001";

/** The model of a language's words: read once, then asked about any word. */
export function likenessModel(words: readonly Word[], limit = WORDS_READ): Model {
  const model: Model = { words: new Map(), single: new Map(), singleTotal: [], pair: new Map(), pairTotal: [], holes: new Map(), dropped: new Map(), whole: new Map() };
  const add = (map: Map<string, string[]>, k: string, word: string) => {
    const list = map.get(k) ?? [];
    list.push(word);
    map.set(k, list);
  };
  for (const w of [...words].sort((a, b) => b.count - a.count).slice(0, limit)) {
    const key = fold(w.word);
    const s = sounds(key);
    if (s.length === 0) continue;
    const had = model.words.get(key);
    if (had !== undefined) {
      had.count += w.count;
      continue;
    }
    model.words.set(key, { sounds: s, count: w.count, word: w.word });
  }
  for (const [key, w] of model.words) {
    const wt = weight(w.count);
    w.sounds.forEach((x, i) => {
      model.single.set(`${i}${SEP}${x}`, (model.single.get(`${i}${SEP}${x}`) ?? 0) + wt);
      model.singleTotal[i] = (model.singleTotal[i] ?? 0) + wt;
      if (i + 1 < w.sounds.length) {
        const k = `${i}${SEP}${x}${SEP}${w.sounds[i + 1]}`;
        model.pair.set(k, (model.pair.get(k) ?? 0) + wt);
        model.pairTotal[i] = (model.pairTotal[i] ?? 0) + wt;
      }
    });
    for (const hole of holesOf(w.sounds)) add(model.holes, hole, key);
    for (const d of droppedOf(w.sounds)) add(model.dropped, d, key);
    add(model.whole, w.sounds.join(SEP), key);
  }
  return model;
}

/** The sounds with each one in turn replaced by a hole, marked by where it was. */
function holesOf(s: readonly string[]): string[] {
  return s.map((_, i) => `${i}${SEP}${[...s.slice(0, i), "_", ...s.slice(i + 1)].join(SEP)}`);
}

/** The sounds with each one in turn left out. */
function droppedOf(s: readonly string[]): string[] {
  return [...new Set(s.map((_, i) => [...s.slice(0, i), ...s.slice(i + 1)].join(SEP)))];
}

export type Likeness = {
  word: string;
  sounds: string[];
  /** Is it a word of the language (as written in the texts), and how often is it used. */
  known: number;
  neighbours: string[];
  /** Sums over its positions (Vitevitch and Luce 2004): higher is more typical. */
  positional: number;
  biphone: number;
};

/** How typical `word`'s sounds are, by the model. */
export function likeness(model: Model, word: string): Likeness {
  const key = fold(word);
  const s = sounds(key);
  const known = model.words.get(key)?.count ?? 0;
  // A sound changed: the same holes. A sound added: longer words that, one
  // sound left out, are this word. A sound left out: shorter words that are
  // this word with one sound left out.
  const near = new Set<string>();
  for (const hole of holesOf(s)) for (const other of model.holes.get(hole) ?? []) near.add(other);
  for (const other of model.dropped.get(s.join(SEP)) ?? []) near.add(other);
  for (const d of droppedOf(s)) for (const other of model.whole.get(d) ?? []) near.add(other);
  near.delete(key);
  const positional = s.reduce((sum, x, i) => sum + ((model.single.get(`${i}${SEP}${x}`) ?? 0) / (model.singleTotal[i] || 1)), 0);
  let biphone = 0;
  for (let i = 0; i + 1 < s.length; i++) biphone += (model.pair.get(`${i}${SEP}${s[i]}${SEP}${s[i + 1]}`) ?? 0) / (model.pairTotal[i] || 1);
  const neighbours = [...near].sort((a, b) => (model.words.get(b)?.count ?? 0) - (model.words.get(a)?.count ?? 0)).map((k) => model.words.get(k)?.word ?? k);
  return { word, sounds: s, known, neighbours, positional, biphone };
}

/** One word's result in words. */
export function likenessLine(l: Likeness): string {
  if (l.sounds.length === 0) return "Type a word to see how its sounds sit in this language.";
  const known = l.known > 0 ? `A word of the texts, used ${l.known} ${l.known === 1 ? "time" : "times"}.` : "Not a word of the texts.";
  const n = l.neighbours.length;
  const near = n === 0 ? "No word is one sound away." : `${n} ${n === 1 ? "word is" : "words are"} one sound away (its neighbourhood density).`;
  return `${known} ${near} Positional probability ${l.positional.toFixed(3)}, biphone ${l.biphone.toFixed(4)}.`;
}

/** Every word type's measures, the commonest first, tab-separated for a stimulus list. */
export function likenessTsv(model: Model): string {
  const rows = [...model.words.values()].sort((a, b) => b.count - a.count).map((w) => {
    const l = likeness(model, w.word);
    return [w.word, String(w.count), String(l.sounds.length), String(l.neighbours.length), l.positional.toFixed(4), l.biphone.toFixed(5)].join("\t");
  });
  return ["word\tuses\tsounds\tneighbours\tpositional_probability\tbiphone_probability", ...rows].join("\n") + "\n";
}
