import assert from "node:assert/strict";
import { test } from "node:test";
import { bundle, featuresOf, featuresTsv, matching, nextValue, type Feature } from "./features.ts";

test("consonants and vowels get their features from the chart", () => {
  const b = featuresOf("b")!;
  assert.deepEqual([b.syllabic, b.consonantal, b.sonorant, b.continuant, b["delayed release"], b.voice, b.labial, b.coronal, b.dorsal, b.high], ["-", "+", "-", "-", "-", "+", "+", "-", "-", ""]);
  const s = featuresOf("s")!;
  assert.deepEqual([s.continuant, s["delayed release"], s.voice, s.coronal], ["+", "+", "-", "+"]);
  const n = featuresOf("n")!;
  assert.deepEqual([n.sonorant, n.nasal, n.continuant, n["delayed release"]], ["+", "+", "-", ""]);
  const l = featuresOf("l")!;
  assert.deepEqual([l.lateral, l.sonorant, l.consonantal], ["+", "+", "+"]);
  const j = featuresOf("j")!;
  assert.deepEqual([j.consonantal, j.sonorant, j.high, j.back], ["-", "+", "+", "-"]);
  const k = featuresOf("k")!;
  assert.deepEqual([k.dorsal, k.high, k.back, k.low], ["+", "+", "+", "-"]);
  const q = featuresOf("q")!;
  assert.equal(q.high, "-");
  const i = featuresOf("i")!;
  assert.deepEqual([i.syllabic, i.high, i.low, i.back, i.round], ["+", "+", "-", "-", "-"]);
  const u = featuresOf("u")!;
  assert.deepEqual([u.back, u.round, u.labial], ["+", "+", "+"]);
  const a = featuresOf("a")!;
  assert.equal(a.low, "+");
  assert.equal(featuresOf("ã")?.nasal, "+");
  const tsh = featuresOf("t͡ʃ")!;
  assert.deepEqual([tsh.continuant, tsh["delayed release"], tsh.voice, tsh.coronal], ["-", "+", "-", "+"]);
  assert.equal(featuresOf("ǃ"), null);
});

test("picked values pick out the natural class", () => {
  const inv = ["p", "t", "k", "b", "d", "g", "s", "z", "m", "n", "l", "i", "u", "a", "ǃ"];
  const picked = new Map<Feature, "+" | "-">([["voice", "+"], ["sonorant", "-"]]);
  assert.deepEqual(matching(inv, picked), ["b", "d", "g", "z"]);
  assert.equal(bundle(picked), "[+voice, −sonorant]");
  assert.deepEqual(matching(inv, new Map([["syllabic", "+"], ["high", "+"]])), ["i", "u"]);
  assert.deepEqual([nextValue(undefined), nextValue("+"), nextValue("-")], ["+", "-", undefined]);
  const tsv = featuresTsv(["p", "ǃ"]).split("\n");
  assert.equal(tsv.length, 3);
  assert.match(tsv[1]!, /^p\t−\t\+\t−/);
});
