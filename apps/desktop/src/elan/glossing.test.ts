import assert from "node:assert/strict";
import { test } from "node:test";
import { readGlossingLines, tierGlossingLines } from "./glossing.ts";

test("the tiers in words", () => {
  const g = { index: 0, names: ["tx@A", "words@A", "mb@A", "ge@A"], hasMorphs: true, hasGlosses: true, kept: ["ps@A"], dropped: [], timed: false };
  const lines = tierGlossingLines(g);
  assert.match(lines[0], /^Its tiers «words@A», «mb@A» and «ge@A» hold the words, morphs and glosses of «tx@A»\./u);
  assert.equal(lines[1], "«ps@A» is kept on each word or morph.");
  const timed = tierGlossingLines({ ...g, names: ["utterance", "word"], hasMorphs: false, hasGlosses: false, kept: [], dropped: ["note", "pos"], timed: true });
  assert.match(timed[0], /^Its tiers «word» hold the words of «utterance»/u);
  assert.deepEqual(timed.slice(1), ["The words keep their order but not their times.", "«note» and «pos» are left out."]);
});

test("what reading did", () => {
  assert.deepEqual(readGlossingLines({ sentences: 2, words: 3, morphs: 4, glossed: 4, kept: 0, stray: 0 }), ["Read 3 words in 2 sentences, with 4 morphs (4 glossed)."]);
  assert.deepEqual(readGlossingLines({ sentences: 1, words: 1, morphs: 0, glossed: 0, kept: 1, stray: 2 }), ["Read 1 word in 1 sentence.", "1 sentence already had words and was left as it was.", "2 annotations belonged to no sentence and were left out."]);
});
