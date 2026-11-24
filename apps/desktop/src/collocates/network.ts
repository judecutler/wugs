// A collocation network, as LancsBox's GraphColl draws one: the word in the
// middle, its strongest collocates around it (stronger nearer), and with a
// second word, the collocates the two share between them. Pure layout: no DOM.
import type { CollocateRow } from "./logic.ts";

export type NetNode = { id: string; label: string; x: number; y: number; kind: "centre" | "shared" | "own"; strength: number };
export type NetEdge = { from: string; to: string; width: number };
export type Network = { nodes: NetNode[]; edges: NetEdge[] };

/** How many collocates of each word are drawn. */
export const PER_WORD = 14;

const fold = (s: string) => s.normalize("NFC").toLowerCase();

/** The strongest collocates by logDice, ones rarer together than chance left out. */
function strongest(rows: readonly CollocateRow[], k: number, words: readonly string[]): CollocateRow[] {
  const centres = new Set(words.map(fold));
  return rows
    .filter((r) => r.logLikelihood > 0 && !centres.has(fold(r.item)))
    .sort((a, b) => b.logDice - a.logDice || b.together - a.together)
    .slice(0, k);
}

/** 0 for the weakest drawn, 1 for the strongest. */
function scale(values: readonly number[]): (v: number) => number {
  const lo = Math.min(...values);
  const hi = Math.max(...values);
  return (v) => (hi > lo ? (v - lo) / (hi - lo) : 1);
}

/** One word with its collocates around it, or two with the shared ones between. */
export function network(centres: readonly { word: string; rows: readonly CollocateRow[] }[], k = PER_WORD): Network {
  const nodes: NetNode[] = [];
  const edges: NetEdge[] = [];
  // Neither word is drawn as the other's collocate: it is a centre already.
  const words = centres.slice(0, 2).map((c) => c.word);
  const picked = centres.slice(0, 2).map((c) => ({ word: c.word, rows: strongest(c.rows, k, words) }));
  const all = picked.flatMap((c) => c.rows);
  if (all.length === 0 && picked.length === 0) return { nodes, edges };
  const near = scale(all.map((r) => r.logDice));
  const thick = scale(all.map((r) => r.together));
  const width = (r: CollocateRow) => 1 + 3 * thick(r.together);
  if (picked.length === 1) {
    const c = picked[0]!;
    nodes.push({ id: "c0", label: c.word, x: 0, y: 0, kind: "centre", strength: 1 });
    c.rows.forEach((r, i) => {
      const angle = (i / Math.max(1, c.rows.length)) * 2 * Math.PI - Math.PI / 2;
      const radius = 0.35 + 0.6 * (1 - near(r.logDice));
      const id = `n:${fold(r.item)}`;
      nodes.push({ id, label: r.item, x: radius * Math.cos(angle), y: radius * Math.sin(angle), kind: "own", strength: near(r.logDice) });
      edges.push({ from: "c0", to: id, width: width(r) });
    });
    return { nodes, edges };
  }
  const [a, b] = picked as [(typeof picked)[0], (typeof picked)[0]];
  nodes.push({ id: "c0", label: a.word, x: -0.9, y: 0, kind: "centre", strength: 1 }, { id: "c1", label: b.word, x: 0.9, y: 0, kind: "centre", strength: 1 });
  const inB = new Map(b.rows.map((r) => [fold(r.item), r]));
  const shared = a.rows.filter((r) => inB.has(fold(r.item)));
  const sharedKeys = new Set(shared.map((r) => fold(r.item)));
  // Shared ones between the two, nearer the word they keep closer company with.
  shared.forEach((r, i) => {
    const rb = inB.get(fold(r.item))!;
    const id = `n:${fold(r.item)}`;
    const lean = Math.max(-0.5, Math.min(0.5, (rb.logDice - r.logDice) / 4));
    const y = shared.length === 1 ? 0 : -0.95 + (1.9 * i) / (shared.length - 1);
    nodes.push({ id, label: r.item, x: lean * 0.6, y, kind: "shared", strength: (near(r.logDice) + near(rb.logDice)) / 2 });
    edges.push({ from: "c0", to: id, width: width(r) }, { from: "c1", to: id, width: width(rb) });
  });
  // Each word's own on its outer side.
  for (const [side, c] of [[0, a], [1, b]] as const) {
    const own = c.rows.filter((r) => !sharedKeys.has(fold(r.item)));
    const cx = side === 0 ? -0.9 : 0.9;
    own.forEach((r, i) => {
      const t = own.length === 1 ? 0.5 : i / (own.length - 1);
      const angle = side === 0 ? Math.PI / 2 + t * Math.PI : -Math.PI / 2 + t * Math.PI;
      const radius = 0.3 + 0.4 * (1 - near(r.logDice));
      const id = `n${side}:${fold(r.item)}`;
      nodes.push({ id, label: r.item, x: cx + radius * Math.cos(angle), y: radius * Math.sin(angle) * 1.4, kind: "own", strength: near(r.logDice) });
      edges.push({ from: `c${side}`, to: id, width: width(r) });
    });
  }
  return { nodes, edges };
}
