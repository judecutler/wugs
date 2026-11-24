import { test } from "node:test";
import assert from "node:assert/strict";
import { layout, ldndMatrix, midpointRoot, neighbourJoining, newick, phylip, treeLine, type Lect, type Node } from "./tree.ts";
import type { Row } from "./logic.ts";

// The worked example of neighbour joining (Saitou and Nei's method as Wikipedia gives it): an additive matrix.
const NAMES = ["a", "b", "c", "d", "e"];
const D = [
  [0, 5, 9, 9, 8],
  [5, 0, 10, 10, 9],
  [9, 10, 0, 8, 7],
  [9, 10, 8, 0, 3],
  [8, 9, 7, 3, 0],
];

/** Every leaf's distance from every other along the tree. */
function patristic(tree: Node): Map<string, number> {
  const out = new Map<string, number>();
  const leaves = (n: Node, d: number): [string, number][] => (n.children.length === 0 ? [[n.name!, d]] : n.children.flatMap((c) => leaves(c, d + c.length)));
  const walk = (n: Node) => {
    const parts = n.children.map((c) => leaves(c, c.length));
    for (let i = 0; i < parts.length; i++)
      for (let j = i + 1; j < parts.length; j++)
        for (const [x, dx] of parts[i]!) for (const [y, dy] of parts[j]!) out.set([x, y].sort().join(""), dx + dy);
    n.children.forEach(walk);
  };
  walk(tree);
  return out;
}

test("neighbour joining gives back an additive matrix exactly", () => {
  const tree = neighbourJoining(NAMES, D);
  const p = patristic(tree);
  for (let i = 0; i < 5; i++) for (let j = i + 1; j < 5; j++) assert.ok(Math.abs(p.get(NAMES[i]! + NAMES[j]!)! - D[i]![j]!) < 1e-9, `${NAMES[i]}${NAMES[j]}`);
});

test("the midpoint root keeps every distance and halves the longest path", () => {
  const rooted = midpointRoot(neighbourJoining(NAMES, D));
  const p = patristic(rooted);
  for (let i = 0; i < 5; i++) for (let j = i + 1; j < 5; j++) assert.ok(Math.abs(p.get(NAMES[i]! + NAMES[j]!)! - D[i]![j]!) < 1e-9);
  assert.equal(rooted.children.length, 2);
  const deepest = (n: Node): number => n.length + Math.max(0, ...n.children.map(deepest));
  assert.ok(Math.abs(deepest(rooted.children[0]!) - deepest(rooted.children[1]!)) < 1e-9, "both sides reach as far");
  assert.ok(Math.abs(deepest(rooted.children[0]!) - 5) < 1e-9, "half of the longest path, 10");
});

test("Newick, PHYLIP and the layout", () => {
  const rooted = midpointRoot(neighbourJoining(["North Tongue", "b", "c"], [[0, 2, 4], [2, 0, 4], [4, 4, 0]]));
  const nwk = newick(rooted);
  assert.match(nwk, /^\(.*\);\n$/u);
  assert.match(nwk, /North_Tongue:1\.0000/u);
  assert.equal(phylip({ names: ["North Tongue", "b"], d: [[0, 0.5], [0.5, 0]], compared: [], left: [] }), "2\nNorth_Tong 0.0000 0.5000\nb          0.5000 0.0000\n");
  const drawn = layout(rooted);
  assert.equal(drawn.leaves.length, 3);
  assert.ok(Math.abs(drawn.depth - 2) < 1e-9, "half the longest path, 4");
});

test("languages sharing too few concepts are left out of the matrix", () => {
  const words = ["mata", "kulu", "nanu", "bira", "selo", "tumi", "kapa", "luwe", "dori", "fenu", "gasi", "hoto"];
  const rows = words.map((w, i) => ({ concept: { id: String(i), gloss: String(i) }, ours: [w], others: new Map() }) as unknown as Row);
  const shift = (w: string) => w.replace("a", "e");
  const lects: Lect[] = [
    { name: "Here", forms: (r) => r.ours },
    { name: "Near", forms: (r) => r.ours.map(shift) },
    { name: "Far", forms: (r) => r.ours.map((w) => [...w].reverse().join("")) },
    { name: "Thin", forms: (r) => (Number(r.concept.id) < 3 ? r.ours : []) },
  ];
  const m = ldndMatrix(rows, lects);
  assert.deepEqual(m.names, ["Here", "Near", "Far"]);
  assert.deepEqual(m.left, ["Thin"]);
  assert.ok(m.d[0]![1]! < m.d[0]![2]!, "a sound shift is nearer than a reversal");
  assert.match(treeLine(m), /3 languages.*Thin/u);
  assert.match(treeLine({ names: ["a", "b"], d: [], compared: [], left: ["c"] }), /Too few/u);
});
