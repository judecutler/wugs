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
export type Affixes = { affixes: Affix[]; words: number; withoutRoot: number };

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
