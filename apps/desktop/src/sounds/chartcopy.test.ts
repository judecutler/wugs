import assert from "node:assert/strict";
import { test } from "node:test";
import { chartClipboard, chartRows } from "./chartcopy.ts";
import { chart, inventory } from "./ipa.ts";

const c = chart(inventory([{ word: "pata", count: 3 }, { word: "bika", count: 2 }, { word: "t͡ʃumu", count: 1 }]));

test("the chart as rows for a paper", () => {
  const { rows, vowels, others } = chartRows(c);
  assert.deepEqual(rows[0], ["", "Bilabial", "Alveolar", "Velar"]);
  assert.deepEqual(rows[1], ["Plosive", "p b", "t", "k"]);
  assert.deepEqual(rows[2], ["Nasal", "m", "", ""]);
  assert.equal(vowels, "i u a");
  assert.equal(others, "t͡ʃ");
  const word = chartClipboard(c, "word");
  assert.match(word.html!, /^<table style="border-collapse:collapse"><tr><th /);
  assert.ok(word.text.includes("Plosive\tp b\tt\tk"));
  assert.ok(word.text.endsWith("Vowels: i u a\nAlso: t͡ʃ\n"));
  const tex = chartClipboard(c, "expex").text.split("\n");
  assert.equal(tex[0], "\\begin{tabular}{|c|c|c|c|}");
  assert.equal(tex[3], "Plosive & p b & t & k \\\\ \\hline");
  assert.equal(chartClipboard(c, "text").text.split("\n")[1], "Plosive  p b       t         k");
});
