// Clusters (AntConc's Clusters tool): the word sequences of a given length
// that hold the item found, counted over the Concordance's lines. Pure: no DOM.

import type { Hit } from "./logic";

/** Where the item stands in a cluster: first, last or anywhere. */
export type ClusterSide = "right" | "left" | "any";

export const CLUSTER_SIDES: { id: ClusterSide; label: string }[] = [
  { id: "right", label: "Item first (it and the words after)" },
  { id: "left", label: "Item last (the words before and it)" },
  { id: "any", label: "Item anywhere in it" },
];

export const CLUSTER_SIZES = [2, 3, 4, 5] as const;

export type Cluster = {
  /** The spelling met most often, word by word. */
  words: string[];
  /** Where the item starts and how long it is, in that spelling. */
  at: number;
  len: number;
  /** Lines it occurs in (a line counts once per place it fits). */
  count: number;
  texts: number;
};

/** Every cluster of `size` words holding the whole item, commonest first.
 * Case is ignored and diacritics are not, as the Concordance compares. */
export function clusters(hits: readonly Hit[], size: number, side: ClusterSide): Cluster[] {
  type Acc = { count: number; texts: Set<string>; spellings: Map<string, { n: number; words: string[]; at: number; len: number }> };
  const found = new Map<string, Acc>();
  for (const hit of hits) {
    const len = Math.max(1, hit.len ?? 1);
    if (len > size) continue;
    const lo = Math.max(0, hit.at + len - size);
    const hi = Math.min(hit.at, hit.words.length - size);
    const starts = side === "right" ? [hit.at] : side === "left" ? [hit.at + len - size] : Array.from({ length: Math.max(0, hi - lo + 1) }, (_, i) => lo + i);
    for (const s of starts) {
      if (s < 0 || s + size > hit.words.length) continue;
      const words = hit.words.slice(s, s + size);
      const key = words.map((w) => w.toLocaleLowerCase()).join("\u001f");
      const acc = found.get(key) ?? { count: 0, texts: new Set<string>(), spellings: new Map() };
      acc.count += 1;
      acc.texts.add(hit.textId);
      const spelled = words.join("\u001f");
      const sp = acc.spellings.get(spelled) ?? { n: 0, words, at: hit.at - s, len };
      sp.n += 1;
      acc.spellings.set(spelled, sp);
      found.set(key, acc);
    }
  }
  return [...found.values()]
    .map((acc) => {
      const best = [...acc.spellings.values()].sort((a, b) => b.n - a.n)[0]!;
      return { words: best.words, at: best.at, len: best.len, count: acc.count, texts: acc.texts.size };
    })
    .sort((a, b) => b.count - a.count || a.words.join(" ").localeCompare(b.words.join(" ")));
}

/** The line above the clusters. */
export function clusterLine(found: readonly Cluster[], shown: number, total: number, size: number): string {
  if (shown === 0) return "Nothing found.";
  const of = total > shown ? ` Counted over the first ${shown} of ${total} lines; narrow the search to count them all.` : "";
  if (found.length === 0) return `No line has ${size} words around it.${of}`;
  const kinds = found.length === 1 ? "1 cluster" : `${found.length} different clusters`;
  return `${kinds} of ${size} words.${of}`;
}

/** The clusters as a table to paste into a spreadsheet. */
export function clustersTsv(found: readonly Cluster[]): string {
  return ["cluster\tlines\ttexts", ...found.map((c) => `${c.words.join(" ").replace(/[\t\n]/g, " ")}\t${c.count}\t${c.texts}`)].join("\n");
}
