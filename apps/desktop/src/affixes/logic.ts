// The Affixes panel's words and tables (src-tauri/src/affixes.rs): the
// position-class chart a grammar's morphology chapter draws, each affix in
// the slot it holds most often, and the table behind it. Pure; no DOM.

export type Affix = {
  surface: string;
  kind: string;
  gloss: string;
  uses: number;
  roots: number;
  positions: [number, number][];
  slot: number;
  categories: [string, number][];
  example: { word: string; textId: string; tierId: string; segmentId: string };
};
/** Two affixes met in one word, `first` (an index into `affixes`) before `second`, in `count` words. */
export type Order = { first: number; second: number; count: number };
export type Affixes = { affixes: Affix[]; words: number; withoutRoot: number; orders?: Order[] };

const MARK: Record<string, [string, string]> = { prefix: ["", "-"], suffix: ["-", ""], infix: ["<", ">"], reduplication: ["", "~"] };

/** An affix as a grammar writes it: `ni-`, `-ra`, `<um>`, and a clitic `=ma`
 * or, before the root, `ma=`. */
export function marked(a: Pick<Affix, "surface" | "kind" | "slot">): string {
  if (a.kind === "clitic") return a.slot < 0 ? `${a.surface}=` : `=${a.surface}`;
  const [before, after] = MARK[a.kind] ?? ["", ""];
  return `${before}${a.surface}${after}`;
}

/** −2, −1, Root, +1, +2 … */
export function slotLabel(slot: number): string {
  if (slot === 0) return "Root";
  return slot < 0 ? `−${-slot}` : `+${slot}`;
}

export type Column = { slot: number; affixes: Affix[] };

/** The chart's columns: every slot some affix holds, and the root's,
 * from the farthest prefix slot to the farthest suffix slot; in each,
 * commonest first. */
export function slotChart(affixes: readonly Affix[]): Column[] {
  const slots = new Set<number>([0]);
  for (const a of affixes) slots.add(a.slot);
  return [...slots]
    .sort((a, b) => a - b)
    .map((slot) => ({ slot, affixes: affixes.filter((a) => a.slot === slot).sort((a, b) => b.uses - a.uses || a.surface.localeCompare(b.surface)) }));
}

/** Where else an affix turns up, in words: "also at +2 (×1)". Empty when it
 * keeps to its slot. */
export function elsewhere(a: Affix): string {
  const other = a.positions.filter(([p]) => p !== a.slot);
  if (other.length === 0) return "";
  return `also at ${other.map(([p, n]) => `${slotLabel(p)} (×${n})`).join(", ")}`;
}

export function summaryLine(r: Affixes): string {
  if (r.words === 0) return "No word has been broken into a root and affixes yet. Gloss words in the Gloss panel, or import glossed texts.";
  const n = r.affixes.length;
  const head = `${n} ${n === 1 ? "affix or clitic" : "affixes and clitics"}, read from ${r.words} glossed ${r.words === 1 ? "word" : "words"}.`;
  return r.withoutRoot > 0 ? `${head} ${r.withoutRoot} ${r.withoutRoot === 1 ? "word has" : "words have"} no morph marked as a root and ${r.withoutRoot === 1 ? "is" : "are"} left out.` : head;
}

/** The chart as tab-separated text: a header row of slots, then a row per rank. */
export function chartTsv(columns: readonly Column[]): string {
  const depth = Math.max(1, ...columns.map((c) => c.affixes.length));
  const lines = [columns.map((c) => slotLabel(c.slot)).join("\t")];
  for (let i = 0; i < depth; i++) {
    lines.push(columns.map((c) => (c.slot === 0 && i === 0 ? "ROOT" : c.affixes[i] ? `${marked(c.affixes[i]!)} ${c.affixes[i]!.gloss}`.trim() : "")).join("\t"));
  }
  return lines.join("\n") + "\n";
}

/** Every affix as a table row, for a spreadsheet. */
export function affixTsv(affixes: readonly Affix[]): string {
  const head = ["affix", "gloss", "type", "slot", "uses", "roots", "elsewhere", "root categories", "example"];
  const rows = affixes.map((a) => [marked(a), a.gloss, a.kind, slotLabel(a.slot), String(a.uses), String(a.roots), elsewhere(a), a.categories.map(([c, n]) => `${c} ${n}`).join("; "), a.example.word]);
  return [head, ...rows].map((r) => r.map((c) => c.replace(/[\t\n]/g, " ")).join("\t")).join("\n") + "\n";
}

/** Two affixes found together, with how often each order is met. `a` is
 * the one the chart puts first (the lower slot), so `ab` is the chart's
 * order and `ba` the other. */
export type Pair = { a: Affix; b: Affix; ab: number; ba: number; sameSlot: boolean };

/** Every two affixes met in one word, the orders folded together: most
 * often together first. */
export function pairs(r: Affixes): Pair[] {
  const by = new Map<string, Pair>();
  for (const o of r.orders ?? []) {
    const x = r.affixes[o.first];
    const y = r.affixes[o.second];
    if (x === undefined || y === undefined) continue;
    // The chart's order: lower slot first, then the backend's order.
    const forward = x.slot < y.slot || (x.slot === y.slot && o.first < o.second);
    const [lo, hi] = forward ? [o.first, o.second] : [o.second, o.first];
    const key = `${lo}:${hi}`;
    let p = by.get(key);
    if (p === undefined) {
      const a = r.affixes[lo]!;
      const b = r.affixes[hi]!;
      p = { a, b, ab: 0, ba: 0, sameSlot: a.slot === b.slot };
      by.set(key, p);
    }
    if (forward) p.ab += o.count;
    else p.ba += o.count;
  }
  return [...by.values()].sort((p, q) => q.ab + q.ba - (p.ab + p.ba) || marked(p.a).localeCompare(marked(q.a)));
}

/** What a pair says about the template, in words. */
export function pairNote(p: Pair): string {
  if (p.sameSlot) return "Both stand in one slot on the chart, yet they occur in one word: one belongs in another slot";
  if (p.ba > 0 && p.ab > 0) return "Both orders are met: check these words, or the order is free";
  if (p.ba > 0) return "Only in the other order: the chart's slots disagree with the words";
  return "";
}

/** A pair as one line: "-ra before =ma ×5, after it ×1". */
export function pairLine(p: Pair): string {
  const head = `${marked(p.a)} before ${marked(p.b)}`;
  if (p.ba === 0) return `${head} ×${p.ab}`;
  if (p.ab === 0) return `${marked(p.b)} before ${marked(p.a)} ×${p.ba}`;
  return `${head} ×${p.ab}, after it ×${p.ba}`;
}

/** The sentence above the pairs: how many there are and how many need a look. */
export function pairsSummary(ps: readonly Pair[]): string {
  if (ps.length === 0) return "No word has two affixes yet, so nothing says how they are ordered.";
  const flagged = ps.filter((p) => pairNote(p) !== "").length;
  const head = `${ps.length} ${ps.length === 1 ? "pair" : "pairs"} of affixes occur in one word.`;
  if (flagged === 0) return `${head} Each always comes in the chart's order.`;
  return `${head} ${flagged} ${flagged === 1 ? "needs" : "need"} a look.`;
}

/** The pairs as a table for a spreadsheet. */
export function pairsTsv(ps: readonly Pair[]): string {
  const head = ["first", "second", "in this order", "in the other order", "note"];
  const rows = ps.map((p) => [`${marked(p.a)} ${p.a.gloss}`.trim(), `${marked(p.b)} ${p.b.gloss}`.trim(), String(p.ab), String(p.ba), pairNote(p)]);
  return [head, ...rows].map((r) => r.map((c) => c.replace(/[\t\n]/g, " ")).join("\t")).join("\n") + "\n";
}
