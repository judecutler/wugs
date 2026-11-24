import { test } from "node:test";
import assert from "node:assert/strict";
import { affixTsv, chartTsv, elsewhere, marked, productivity, productivityLine, slotChart, slotLabel, summaryLine, type Affix } from "./logic.ts";

const ex = { word: "ni-kani", textId: "t", tierId: "r", segmentId: "s" };
const a = (surface: string, kind: string, gloss: string, slot: number, uses: number, positions: [number, number][] = [[slot, uses]]): Affix => ({
  surface,
  kind,
  gloss,
  uses,
  roots: 1,
  positions,
  slot,
  categories: [["v", uses]],
  example: ex,
});

const ni = a("ni", "prefix", "1SG", -1, 4);
const ka = a("ka", "prefix", "2SG", -1, 2);
const ra = a("ra", "suffix", "PST", 1, 3);
const ma = a("ma", "clitic", "FOC", 2, 2, [[2, 1], [1, 1]]);

test("affixes are written with their markers", () => {
  assert.equal(marked(ni), "ni-");
  assert.equal(marked(ra), "-ra");
  assert.equal(marked(ma), "=ma");
  assert.equal(marked({ surface: "ma", kind: "clitic", slot: -1 }), "ma=");
  assert.equal(marked({ surface: "um", kind: "infix", slot: 1 }), "<um>");
  assert.equal(slotLabel(-2), "−2");
  assert.equal(slotLabel(0), "Root");
  assert.equal(slotLabel(3), "+3");
});

test("the chart has a column per slot, the root in its place", () => {
  const chart = slotChart([ra, ma, ka, ni]);
  assert.deepEqual(chart.map((c) => c.slot), [-1, 0, 1, 2]);
  assert.deepEqual(chart[0]!.affixes.map((x) => x.surface), ["ni", "ka"], "commonest first");
  assert.equal(chart[1]!.affixes.length, 0);
  assert.equal(chartTsv(chart), "−1\tRoot\t+1\t+2\nni- 1SG\tROOT\t-ra PST\t=ma FOC\nka- 2SG\t\t\t\n");
  assert.deepEqual(slotChart([]).map((c) => c.slot), [0]);
});

test("the table and summary say it in words", () => {
  assert.equal(elsewhere(ma), "also at +1 (×1)");
  assert.equal(elsewhere(ni), "");
  assert.match(affixTsv([ni]), /^affix\tgloss\ttype\tslot.*\nni-\t1SG\tprefix\t−1\t4\t1\t\t\t\t\tv 4\tni-kani\n$/s);
  assert.match(affixTsv([{ ...ni, types: 3, hapaxes: 2 }]), /\t4\t1\t3\t2\t0\.500\t/);
  assert.equal(summaryLine({ affixes: [ni, ra], words: 5, withoutRoot: 0 }), "2 affixes and clitics, read from 5 glossed words.");
  assert.match(summaryLine({ affixes: [], words: 0, withoutRoot: 0 }), /No word/);
  assert.match(summaryLine({ affixes: [ni], words: 1, withoutRoot: 2 }), /2 words have no morph marked as a root and are left out/);
});

test("pairs fold both orders and flag what disagrees with the chart", async () => {
  const { pairs, pairLine, pairNote, pairsSummary, pairsTsv } = await import("./logic.ts");
  const ex = { word: "", textId: "", tierId: "", segmentId: "" };
  const a = (surface: string, kind: string, gloss: string, slot: number) => ({ surface, kind, gloss, uses: 1, roots: 1, positions: [[slot, 1]] as [number, number][], slot, categories: [], example: ex });
  const affixes = [a("ni", "prefix", "1SG", -1), a("ka", "prefix", "2SG", -1), a("ra", "suffix", "PST", 1), a("ma", "clitic", "FOC", 2)];
  const r = {
    affixes,
    words: 9,
    withoutRoot: 0,
    orders: [
      { first: 2, second: 3, count: 5 },
      { first: 3, second: 2, count: 1 },
      { first: 0, second: 2, count: 4 },
      { first: 0, second: 1, count: 1 },
    ],
  };
  const ps = pairs(r);
  assert.deepEqual(ps.map((p) => [p.a.surface, p.b.surface, p.ab, p.ba]), [["ra", "ma", 5, 1], ["ni", "ra", 4, 0], ["ni", "ka", 1, 0]]);
  assert.equal(pairLine(ps[0]!), "-ra before =ma ×5, after it ×1");
  assert.equal(pairLine(ps[1]!), "ni- before -ra ×4");
  assert.match(pairNote(ps[0]!), /Both orders/);
  assert.equal(pairNote(ps[1]!), "");
  assert.match(pairNote(ps[2]!), /one slot/);
  assert.equal(pairsSummary(ps), "3 pairs of affixes occur in one word. 2 need a look.");
  assert.match(pairsSummary([]), /No word has two affixes/);
  // Only met against the chart's order.
  const back = pairs({ ...r, orders: [{ first: 3, second: 2, count: 2 }] });
  assert.equal(pairLine(back[0]!), "=ma before -ra ×2");
  assert.match(pairNote(back[0]!), /other order/);
  assert.equal(pairsTsv(ps).split("\n")[1], "-ra PST\t=ma FOC\t5\t1\tBoth orders are met: check these words, or the order is free");
  assert.deepEqual(pairs({ affixes, words: 1, withoutRoot: 0 }), [], "an older backend sends no orders");
});

test("productivity is the share of uses in word forms met once", () => {
  const a = (surface: string, uses: number, hapaxes: number): Affix => ({ surface, kind: "suffix", gloss: "X", uses, roots: 1, positions: [[1, uses]], slot: 1, categories: [], example: { word: "", textId: "", tierId: "", segmentId: "" }, types: hapaxes + 1, hapaxes });
  assert.equal(productivity(a("ra", 40, 10)), 0.25);
  assert.equal(productivity({ ...a("ra", 40, 10), hapaxes: undefined }), null);
  assert.equal(productivityLine([a("ra", 40, 10), a("si", 20, 12), a("ka", 5, 5)]), "Most productive (Baayen's P, the share of an affix's uses in word forms met only once; affixes used 10 times or more): -si X 0.600, -ra X 0.250.");
  assert.equal(productivityLine([a("ka", 5, 5)]), "");
});
