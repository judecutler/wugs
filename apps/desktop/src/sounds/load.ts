// Functional load: how much the words rely on telling two sounds apart.
// Following Hockett (1967) and Surendran and Niyogi (2006), it is the share
// of the word distribution's entropy that would be lost if the two sounds
// fell together: each word is weighted by how often it is used, the sounds
// are merged, and words that become the same count as one. A contrast with
// many minimal pairs among common words carries a high load; one heard in
// a single rare pair carries almost none. Pure and npm-tested.
import { sounds } from "./ipa.ts";
import { WORDS_READ, type Contrast } from "./pairs.ts";

const SEP = "\u001F";

/** The words cut once, so each contrast costs only the words holding its sounds. */
export type LoadModel = {
  keys: string[];
  cut: string[][];
  counts: number[];
  total: number;
  /** Entropy in bits of the word distribution. */
  entropy: number;
  /** Each word key to its index. */
  index: Map<string, number>;
  /** Each sound to the words it is in. */
  holding: Map<string, number[]>;
};

/** Cut the commonest `limit` word types into sounds; spellings with the same sounds count as one word. */
export function loadModel(words: readonly { word: string; count: number }[], limit = WORDS_READ): LoadModel {
  const read = [...words].filter((w) => w.count > 0).sort((x, y) => y.count - x.count).slice(0, limit);
  const m: LoadModel = { keys: [], cut: [], counts: [], total: 0, entropy: 0, index: new Map(), holding: new Map() };
  for (const { word, count } of read) {
    const s = sounds(word);
    if (s.length === 0) continue;
    const key = s.join(SEP);
    const at = m.index.get(key);
    m.total += count;
    if (at !== undefined) {
      m.counts[at] = (m.counts[at] ?? 0) + count;
      continue;
    }
    const i = m.keys.length;
    m.index.set(key, i);
    m.keys.push(key);
    m.cut.push(s);
    m.counts.push(count);
    for (const x of new Set(s)) {
      const list = m.holding.get(x) ?? [];
      list.push(i);
      m.holding.set(x, list);
    }
  }
  for (const c of m.counts) m.entropy -= term(c, m.total);
  return m;
}

/** p log2 p for a count out of the total. */
function term(count: number, total: number): number {
  if (count <= 0 || total <= 0) return 0;
  const p = count / total;
  return p * Math.log2(p);
}

/**
 * The share (0 to 1) of the words' entropy lost if `a` and `b` fell together,
 * and how many word types would become the same as another.
 */
export function functionalLoad(m: LoadModel, a: string, b: string): { load: number; merged: number } {
  if (m.entropy <= 0 || a === b) return { load: 0, merged: 0 };
  // Only words holding `b` change: each becomes the word with `b` read as `a`.
  // Groups gather those with the word already spelled that way, if any.
  const groups = new Map<string, Set<number>>();
  for (const i of m.holding.get(b) ?? []) {
    const merged = (m.cut[i] ?? []).map((x) => (x === b ? a : x)).join(SEP);
    const group = groups.get(merged) ?? new Set<number>();
    group.add(i);
    const there = m.index.get(merged);
    if (there !== undefined) group.add(there);
    groups.set(merged, group);
  }
  let lost = 0;
  let mergedTypes = 0;
  for (const group of groups.values()) {
    if (group.size < 2) continue;
    mergedTypes += group.size - 1;
    let sum = 0;
    let before = 0;
    for (const i of group) {
      const c = m.counts[i] ?? 0;
      sum += c;
      before += term(c, m.total);
    }
    // Entropy before minus after: -Σ p log p + P log P.
    lost += term(sum, m.total) - before;
  }
  return { load: Math.max(0, lost / m.entropy), merged: mergedTypes };
}

export type Loaded = { a: string; b: string; load: number; merged: number; pairs: number };

/** Every contrast's load, the heaviest first. */
export function loads(m: LoadModel, contrasts: readonly Contrast[]): Loaded[] {
  return contrasts
    .map((c) => ({ a: c.a, b: c.b, pairs: c.total, ...functionalLoad(m, c.a, c.b) }))
    .sort((x, y) => y.load - x.load || y.pairs - x.pairs || x.a.localeCompare(y.a) || x.b.localeCompare(y.b));
}

/** A load as a percentage with enough places to tell small ones apart. */
export function loadLabel(load: number): string {
  const pct = load * 100;
  if (pct === 0) return "0%";
  if (pct < 0.01) return "<0.01%";
  return `${pct < 1 ? pct.toFixed(2) : pct.toFixed(1)}%`;
}

/** The line under a picked contrast, in words. */
export function loadLine(l: { a: string; b: string; load: number; merged: number }): string {
  if (l.merged === 0) return `No two words would fall together if «${l.a}» and «${l.b}» merged.`;
  return `If «${l.a}» and «${l.b}» merged, ${l.merged.toLocaleString("en")} word ${l.merged === 1 ? "type" : "types"} would sound like another, losing ${loadLabel(l.load)} of the information in which word is said (functional load).`;
}

/** The heaviest contrasts in a sentence. */
export function heaviestLine(all: readonly Loaded[], shown = 5): string {
  const top = all.filter((l) => l.load > 0).slice(0, shown);
  if (top.length === 0) return "";
  return `The words lean most on ${top.map((l) => `${l.a} ~ ${l.b} (${loadLabel(l.load)})`).join(", ")}.`;
}

/** Every contrast's load as a table for a spreadsheet. */
export function loadsTsv(all: readonly Loaded[]): string {
  return `${["sound a", "sound b", "minimal pairs", "word types merged", "functional load"].join("\t")}\n${all.map((l) => [l.a, l.b, l.pairs, l.merged, l.load.toFixed(6)].join("\t")).join("\n")}\n`;
}
