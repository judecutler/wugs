import { test } from "node:test";
import assert from "node:assert/strict";
import { affixTsv, chartTsv, elsewhere, marked, slotChart, slotLabel, summaryLine, type Affix } from "./logic.ts";

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
  assert.match(affixTsv([ni]), /^affix\tgloss\ttype\tslot.*\nni-\t1SG\tprefix\t−1\t4\t1\t\tv 4\tni-kani\n$/s);
  assert.equal(summaryLine({ affixes: [ni, ra], words: 5, withoutRoot: 0 }), "2 affixes and clitics, read from 5 glossed words.");
  assert.match(summaryLine({ affixes: [], words: 0, withoutRoot: 0 }), /No word/);
  assert.match(summaryLine({ affixes: [ni], words: 1, withoutRoot: 2 }), /2 words have no morph marked as a root and are left out/);
});
