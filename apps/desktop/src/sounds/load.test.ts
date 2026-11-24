import assert from "node:assert/strict";
import { test } from "node:test";
import { functionalLoad, heaviestLine, loadLabel, loadLine, loadModel, loads, loadsTsv } from "./load.ts";
import { minimalPairs } from "./pairs.ts";

const words = [
  { word: "pata", count: 10 },
  { word: "bata", count: 10 },
  { word: "kata", count: 5 },
  { word: "mo", count: 0 },
];

test("functional load is the share of the words' entropy lost when two sounds merge", () => {
  const m = loadModel(words);
  // 10, 10, 5 of 25: H = 1.5219 bits; p and b merged leave 20, 5: 0.7219 bits.
  assert.ok(Math.abs(m.entropy - 1.52193) < 1e-4);
  const pb = functionalLoad(m, "b", "p");
  assert.equal(pb.merged, 1);
  assert.ok(Math.abs(pb.load - 0.8 / 1.52193) < 1e-4);
  // Either way round is the same contrast.
  assert.ok(Math.abs(functionalLoad(m, "p", "b").load - pb.load) < 1e-12);
  // k and p tell apart the rarer kata: a smaller load.
  const kp = functionalLoad(m, "k", "p");
  assert.ok(kp.load > 0 && kp.load < pb.load);
  // Sounds no two words differ in carry nothing.
  assert.deepEqual(functionalLoad(m, "t", "m"), { load: 0, merged: 0 });
  assert.deepEqual(functionalLoad(m, "a", "a"), { load: 0, merged: 0 });
});

test("two words that fall together with a third merge as one group", () => {
  const m = loadModel([{ word: "pa", count: 1 }, { word: "ba", count: 1 }, { word: "bo", count: 2 }]);
  // b read as p: ba becomes pa; bo becomes po, which no word is.
  const l = functionalLoad(m, "p", "b");
  assert.equal(l.merged, 1);
  assert.ok(l.load > 0);
});

test("contrasts are ranked by load and told in words", () => {
  const m = loadModel(words);
  const all = loads(m, minimalPairs(words));
  // b and p are as common as each other, so b ~ k and k ~ p tie and go by sound.
  assert.deepEqual(all.map((l) => `${l.a}~${l.b}`), ["b~p", "b~k", "k~p"]);
  assert.equal(loadLabel(0), "0%");
  assert.equal(loadLabel(0.00001), "<0.01%");
  assert.equal(loadLabel(0.0042), "0.42%");
  assert.equal(loadLabel(0.5257), "52.6%");
  assert.match(heaviestLine(all), /^The words lean most on b ~ p \(52\.6%\)/);
  assert.equal(heaviestLine([]), "");
  assert.match(loadLine({ a: "b", b: "p", load: 0.5257, merged: 1 }), /1 word type would sound like another, losing 52\.6%/);
  assert.match(loadLine({ a: "x", b: "y", load: 0, merged: 0 }), /No two words/);
  assert.equal(loadsTsv(all).split("\n")[1], `b\tp\t1\t1\t${all[0]!.load.toFixed(6)}`);
});
