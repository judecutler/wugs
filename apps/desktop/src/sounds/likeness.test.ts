import { test } from "node:test";
import assert from "node:assert/strict";
import { likeness, likenessLine, likenessModel, likenessTsv } from "./likeness.ts";

const words = [
  { word: "kata", count: 9 },
  { word: "kita", count: 4 },
  { word: "kat", count: 2 },
  { word: "katan", count: 1 },
  { word: "mata", count: 3 },
  { word: "ŋulu", count: 1 },
];

test("neighbours: a sound changed, added or left out", () => {
  const m = likenessModel(words);
  const l = likeness(m, "kata");
  assert.deepEqual(new Set(l.neighbours), new Set(["kita", "kat", "katan", "mata"]));
  assert.equal(l.known, 9);
  assert.deepEqual(likeness(m, "ŋulu").neighbours, []);
  const made = likeness(m, "pata");
  assert.equal(made.known, 0, "a made-up word");
  assert.deepEqual(new Set(made.neighbours), new Set(["kata", "mata"]));
});

test("a common sound in a common place scores higher", () => {
  const m = likenessModel(words);
  const typical = likeness(m, "kata");
  const odd = likeness(m, "ŋulu");
  assert.ok(typical.positional > odd.positional);
  assert.ok(typical.biphone > odd.biphone);
  // Each position's probabilities sum to at most 1, so a word of n sounds scores at most n.
  assert.ok(typical.positional <= 4);
});

test("in words, and as a table", () => {
  const m = likenessModel(words);
  assert.match(likenessLine(likeness(m, "pata")), /^Not a word of the texts\. 2 words are one sound away/u);
  assert.match(likenessLine(likeness(m, "")), /Type a word/u);
  const tsv = likenessTsv(m).split("\n");
  assert.equal(tsv[0], "word\tuses\tsounds\tneighbours\tpositional_probability\tbiphone_probability");
  assert.match(tsv[1]!, /^kata\t9\t4\t4\t/u);
});
