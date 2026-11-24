import assert from "node:assert/strict";
import { test } from "node:test";
import { indexRows, indicesNote, indicesTsv, synthesisType } from "./greenberg.ts";

const ix = { words: 200, morphs: 520, roots: 210, prefixes: 40, suffixes: 250, infixes: 0, clitics: 20, single: 30 };

test("Greenberg's indices are ratios over the glossed words", () => {
  const rows = indexRows(ix);
  assert.deepEqual(rows.map((r) => [r.ratio, r.value]), [["M/W", 2.6], ["R/W", 1.05], ["P/W", 0.2], ["S/W", 1.25], ["Cl/W", 0.1], ["1/W", 0.15]]);
  assert.deepEqual(indexRows({ ...ix, words: 0 }), []);
  assert.equal(synthesisType(1.06), "analytic");
  assert.equal(synthesisType(2.59), "synthetic");
  assert.equal(synthesisType(3.72), "polysynthetic");
  assert.equal(indicesTsv(ix).split("\n")[1], "Synthesis\tM/W\t2.600\tmorphs per word");
});

test("the note places the language among Greenberg's samples and says which side it affixes on", () => {
  const note = indicesNote(ix);
  assert.match(note, /^Over 200 glossed words, 2\.60 morphs a word: synthetic in Greenberg's \(1960\) terms .*nearest of his samples to Sanskrit \(2\.59\)\./);
  assert.match(note, /Strongly suffixing: 86% of prefixes and suffixes are suffixes\./);
  assert.ok(!note.includes("first look"));
  assert.match(indicesNote({ ...ix, words: 50, morphs: 60, prefixes: 30, suffixes: 2 }), /Strongly prefixing: 94%.*first look/);
  assert.equal(indicesNote({ ...ix, words: 0 }), "No word is broken into morphs yet.");
});
