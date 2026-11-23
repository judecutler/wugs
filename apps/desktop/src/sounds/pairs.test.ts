import { test } from "node:test";
import assert from "node:assert/strict";
import { contrastLabel, contrastsOf, minimalPairs, pairsNote, pairsTable } from "./pairs.ts";

const w = (word: string, count = 1) => ({ word, count });

test("words differing in one sound are minimal pairs for those two sounds", () => {
  const all = minimalPairs([w("pata", 5), w("bata", 3), w("pada", 1), w("tʰa", 2), w("ta", 4), w("kasa")]);
  const pb = all.find((c) => c.a === "b" && c.b === "p");
  assert.deepEqual(pb, { a: "b", b: "p", pairs: [{ wordA: "bata", wordB: "pata", countA: 3, countB: 5 }], total: 1 });
  assert.ok(all.some((c) => c.a === "d" && c.b === "t" && c.pairs[0]?.wordA === "pada"));
  assert.ok(all.some((c) => c.a === "t" && c.b === "tʰ"), "an aspirated stop is a sound of its own");
  assert.ok(!all.some((c) => c.pairs.some((p) => p.wordA === "kasa" || p.wordB === "kasa")));
});

test("words of different lengths or two sounds apart are not pairs", () => {
  assert.deepEqual(minimalPairs([w("pata"), w("bada"), w("pat"), w("pataa")]), []);
});

test("the best attested contrasts come first and pairs are capped", () => {
  const words = ["pa", "ba", "pi", "bi", "pu", "bu", "ma", "na"].map((x) => w(x));
  const all = minimalPairs(words);
  assert.equal(contrastLabel(all[0]!), "b ~ p (3 pairs)");
  assert.deepEqual(contrastsOf(all, "m").map((c) => `${c.a}${c.b}`).sort(), ["bm", "mn", "mp"]);
  assert.equal(contrastsOf(all, null).length, all.length);
  assert.deepEqual(minimalPairs(words, 2).map(contrastLabel), ["b ~ p (1 pair)"], "only the commonest words are read when limited");
});

test("notes and tables say what was found", () => {
  assert.equal(pairsNote([], 0, 0), "No two words here differ in just one sound yet.");
  const all = minimalPairs([w("pa"), w("ba")]);
  assert.equal(pairsNote(all, 2, 2), "1 pair of sounds is told apart by at least one minimal pair.");
  assert.match(pairsNote(all, 2, 9), /among the 2 commonest of 9 word types/u);
  assert.equal(pairsTable(all[0]!), "b\tp\nba\tpa\n");
});
