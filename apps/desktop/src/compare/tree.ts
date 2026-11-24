// A tree of the compared languages, the way ASJP draws them: every pair's
// LDND (distance.ts), joined by neighbour joining (Saitou and Nei 1987) and
// rooted at the midpoint of the longest path. A tree from word lists shows
// how alike the words are, which loans and chance resemblance also make; it
// is a picture of the data, not a proof of history. Pure, npm-tested.
import { sounds } from "../sounds/ipa.ts";
import { MIN_COMPARED, BASELINE_CONCEPTS } from "./distance.ts";
import type { Row } from "./logic.ts";

/** One language: its name and its forms for a row's concept. */
export type Lect = { name: string; forms: (r: Row) => readonly string[] };

export type Matrix = { names: string[]; d: number[][]; compared: number[][]; left: string[] };

/** Concepts read per pair for the tree, fewer than one plot's, as pairs grow with the square of the languages. */
export const TREE_CONCEPTS = 100;
/** At most this many languages in one tree. */
export const MAX_LECTS = 30;

/** A distance over sounds, each form cut into sounds once. */
function distanceOver(): (a: string, b: string) => number {
  const cache = new Map<string, string[]>();
  const cut = (s: string) => {
    let v = cache.get(s);
    if (v === undefined) {
      v = sounds(s.replace(/[-=]/gu, ""));
      cache.set(s, v);
    }
    return v;
  };
  return (a, b) => {
    const x = cut(a);
    const y = cut(b);
    if (x.length === 0 && y.length === 0) return 0;
    let prev = Array.from({ length: y.length + 1 }, (_, j) => j);
    for (let i = 1; i <= x.length; i++) {
      const cur = [i];
      for (let j = 1; j <= y.length; j++) cur[j] = Math.min(prev[j]! + 1, cur[j - 1]! + 1, prev[j - 1]! + (x[i - 1] === y[j - 1] ? 0 : 1));
      prev = cur;
    }
    return prev[y.length]! / Math.max(x.length, y.length);
  };
}

/** LDND between two languages over the concepts both have, with how many those are. */
export function pairLdnd(rows: readonly Row[], a: Lect, b: Lect, dist = distanceOver(), limit = BASELINE_CONCEPTS): { compared: number; ldnd: number | null } {
  const pairs = rows.filter((r) => a.forms(r).length > 0 && b.forms(r).length > 0).slice(0, limit);
  const closest = (x: readonly string[], y: readonly string[]) => Math.min(...x.flatMap((p) => y.map((q) => dist(p, q))));
  if (pairs.length === 0) return { compared: 0, ldnd: null };
  const ldn = pairs.reduce((s, r) => s + closest(a.forms(r), b.forms(r)), 0) / pairs.length;
  let total = 0;
  let n = 0;
  for (let i = 0; i < pairs.length; i++)
    for (let j = 0; j < pairs.length; j++)
      if (i !== j) {
        total += closest(a.forms(pairs[i]!), b.forms(pairs[j]!));
        n += 1;
      }
  const baseline = n === 0 ? 0 : total / n;
  return { compared: pairs.length, ldnd: baseline > 0 ? ldn / baseline : null };
}

/** Every pair's LDND. A language sharing fewer than `MIN_COMPARED` concepts
 * with another is left out (the one with fewest shared first), so every
 * distance in the matrix stands on enough words. */
export function ldndMatrix(rows: readonly Row[], lects: readonly Lect[]): Matrix {
  const dist = distanceOver();
  let keep = lects.slice(0, MAX_LECTS);
  const left = lects.slice(MAX_LECTS).map((l) => l.name);
  const pair = new Map<string, { compared: number; ldnd: number | null }>();
  const get = (a: Lect, b: Lect) => {
    const k = a.name < b.name ? `${a.name}\u0000${b.name}` : `${b.name}\u0000${a.name}`;
    let v = pair.get(k);
    if (v === undefined) {
      v = pairLdnd(rows, a, b, dist, TREE_CONCEPTS);
      pair.set(k, v);
    }
    return v;
  };
  for (;;) {
    const short = keep.map((a) => keep.filter((b) => b !== a && (get(a, b).compared < MIN_COMPARED || get(a, b).ldnd === null)).length);
    const worst = short.reduce((w, n, i) => (n > (short[w] ?? 0) ? i : w), 0);
    if ((short[worst] ?? 0) === 0) break;
    left.push(keep[worst]!.name);
    keep = keep.filter((_, i) => i !== worst);
  }
  const names = keep.map((l) => l.name);
  const d = keep.map((a) => keep.map((b) => (a === b ? 0 : get(a, b).ldnd!)));
  const compared = keep.map((a) => keep.map((b) => (a === b ? 0 : get(a, b).compared)));
  return { names, d, compared, left };
}

/** A tree node: a leaf has a name; `length` is its branch to its parent. */
export type Node = { name?: string; length: number; children: Node[] };

/** Neighbour joining over a distance matrix: an unrooted tree, returned as
 * its last three subtrees joined at one node. Branch lengths below 0 are set to 0. */
export function neighbourJoining(names: readonly string[], d: readonly (readonly number[])[]): Node {
  let nodes: Node[] = names.map((name) => ({ name, length: 0, children: [] }));
  let m = d.map((row) => [...row]);
  while (nodes.length > 3) {
    const n = nodes.length;
    const r = m.map((row) => row.reduce((s, x) => s + x, 0));
    let best = [0, 1];
    let bestQ = Infinity;
    for (let i = 0; i < n; i++)
      for (let j = i + 1; j < n; j++) {
        const q = (n - 2) * m[i]![j]! - r[i]! - r[j]!;
        if (q < bestQ) {
          bestQ = q;
          best = [i, j];
        }
      }
    const [i, j] = best as [number, number];
    const li = Math.max(0, m[i]![j]! / 2 + (r[i]! - r[j]!) / (2 * (n - 2)));
    const lj = Math.max(0, m[i]![j]! - li);
    const joined: Node = { length: 0, children: [{ ...nodes[i]!, length: li }, { ...nodes[j]!, length: lj }] };
    const rest = nodes.map((_, k) => k).filter((k) => k !== i && k !== j);
    const toNew = rest.map((k) => (m[i]![k]! + m[j]![k]! - m[i]![j]!) / 2);
    m = [...rest.map((a, x) => [...rest.map((b) => m[a]![b]!), toNew[x]!]), [...toNew, 0]];
    nodes = [...rest.map((k) => nodes[k]!), joined];
  }
  if (nodes.length === 3) {
    const [a, b, c] = [0, 1, 2];
    const la = Math.max(0, (m[a]![b]! + m[a]![c]! - m[b]![c]!) / 2);
    const lb = Math.max(0, m[a]![b]! - la);
    const lc = Math.max(0, m[a]![c]! - la);
    return { length: 0, children: [{ ...nodes[a]!, length: la }, { ...nodes[b]!, length: lb }, { ...nodes[c]!, length: lc }] };
  }
  if (nodes.length === 2) return { length: 0, children: [{ ...nodes[0]!, length: m[0]![1]! / 2 }, { ...nodes[1]!, length: m[0]![1]! / 2 }] };
  return nodes[0] ?? { length: 0, children: [] };
}

/** The tree rooted at the middle of its longest leaf-to-leaf path. */
export function midpointRoot(tree: Node): Node {
  // An undirected graph of the tree's nodes.
  const edges = new Map<Node, { to: Node; len: number }[]>();
  const link = (a: Node, b: Node, len: number) => {
    edges.set(a, [...(edges.get(a) ?? []), { to: b, len }]);
    edges.set(b, [...(edges.get(b) ?? []), { to: a, len }]);
  };
  const walk = (n: Node) => {
    if (!edges.has(n)) edges.set(n, []);
    for (const c of n.children) {
      link(n, c, c.length);
      walk(c);
    }
  };
  walk(tree);
  const farthest = (from: Node) => {
    const dist = new Map<Node, number>([[from, 0]]);
    const prev = new Map<Node, Node>();
    const stack = [from];
    while (stack.length > 0) {
      const n = stack.pop()!;
      for (const e of edges.get(n) ?? []) {
        if (dist.has(e.to)) continue;
        dist.set(e.to, dist.get(n)! + e.len);
        prev.set(e.to, n);
        stack.push(e.to);
      }
    }
    let far = from;
    for (const [n, x] of dist) if (n.children.length === 0 && x > (dist.get(far) ?? 0)) far = n;
    return { far, dist, prev };
  };
  const leaves = [...edges.keys()].filter((n) => n.children.length === 0);
  if (leaves.length < 2) return tree;
  const a = farthest(leaves[0]!).far;
  const { far: b, dist, prev } = farthest(a);
  const half = dist.get(b)! / 2;
  // Walk back from b to where the middle falls: on the edge (u, v), v nearer a.
  let u = b;
  while (prev.has(u) && dist.get(prev.get(u)!)! > half) u = prev.get(u)!;
  const v = prev.get(u)!;
  const edgeLen = dist.get(u)! - dist.get(v)!;
  const toU = dist.get(u)! - half;
  // Build the rooted tree outward from a new root between u and v.
  const grow = (n: Node, from: Node | null, len: number): Node => {
    const kids = (edges.get(n) ?? []).filter((e) => e.to !== from && !(n === u && e.to === v) && !(n === v && e.to === u)).map((e) => grow(e.to, n, e.len));
    return n.name !== undefined ? { name: n.name, length: len, children: [] } : { length: len, children: kids };
  };
  return { length: 0, children: [grow(u, v, toU), grow(v, u, edgeLen - toU)] };
}

/** A name as Newick allows it unquoted. */
const newickName = (s: string): string => s.replace(/[\s]+/gu, "_").replace(/[(),:;[\]']/gu, "");

/** The tree in Newick, for FigTree, iTOL, SplitsTree or R's ape. */
export function newick(tree: Node): string {
  const write = (n: Node, top: boolean): string => {
    const own = n.children.length === 0 ? newickName(n.name ?? "") : `(${n.children.map((c) => write(c, false)).join(",")})`;
    return top ? own : `${own}:${n.length.toFixed(4)}`;
  };
  return `${write(tree, true)};\n`;
}

/** The matrix in PHYLIP's square format (names cut to 10 characters, as
 * PHYLIP reads them), which SplitsTree and PHYLIP's neighbor read. */
export function phylip(m: Matrix): string {
  const name = (s: string) => newickName(s).slice(0, 10).padEnd(10, " ");
  return [`${m.names.length}`, ...m.names.map((n, i) => `${name(n)} ${m.d[i]!.map((x) => x.toFixed(4)).join(" ")}`)].join("\n") + "\n";
}

export type Drawn = { leaves: { name: string; x: number; y: number }[]; lines: { x1: number; y1: number; x2: number; y2: number }[]; depth: number };

/** A rooted tree laid out as a phylogram: x is the distance from the root,
 * one row per leaf, each inner node halfway between its first and last child. */
export function layout(tree: Node): Drawn {
  const leaves: Drawn["leaves"] = [];
  const lines: Drawn["lines"] = [];
  let depth = 0;
  const place = (n: Node, x0: number): number => {
    const x = x0 + n.length;
    depth = Math.max(depth, x);
    if (n.children.length === 0) {
      const y = leaves.length;
      leaves.push({ name: n.name ?? "", x, y });
      lines.push({ x1: x0, y1: y, x2: x, y2: y });
      return y;
    }
    const ys = n.children.map((c) => place(c, x));
    const y = (Math.min(...ys) + Math.max(...ys)) / 2;
    lines.push({ x1: x, y1: Math.min(...ys), x2: x, y2: Math.max(...ys) });
    if (n.length > 0 || x0 > 0) lines.push({ x1: x0, y1: y, x2: x, y2: y });
    return y;
  };
  place(tree, 0);
  return { leaves, lines, depth };
}

/** The line under the tree. */
export function treeLine(m: Matrix): string {
  const n = m.names.length;
  if (n < 3) return m.left.length > 0 ? `Too few languages share ${MIN_COMPARED} concepts or more with each other to draw a tree.` : "";
  const left = m.left.length === 0 ? "" : ` Left out, too few concepts shared with the others: ${m.left.join(", ")}.`;
  return `A neighbour-joining tree of ${n} languages by LDND, rooted at its midpoint.${left} Alike words come from shared history, but also from loans and chance; read it as a picture of the lists.`;
}
