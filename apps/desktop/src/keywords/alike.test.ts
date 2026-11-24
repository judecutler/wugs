import assert from "node:assert/strict";
import { test } from "node:test";
import { alikeLine, alikeTsv, barShare, nearest, type Alike } from "./alike.ts";

const a: Alike = {
  texts: [
    { id: "a", title: "Fish A", words: 900 },
    { id: "b", title: "Fish B", words: 1200 },
    { id: "c", title: "Songs", words: 300 },
  ],
  mfw: 200,
  similarity: [
    [1, 0.8, -0.6],
    [0.8, 1, -0.5],
    [-0.6, -0.5, 1],
  ],
  omitted: 0,
};

test("the texts nearest one text, most alike first", () => {
  assert.deepEqual(nearest(a, "a").map((n) => [n.title, n.similarity]), [["Fish B", 0.8], ["Songs", -0.6]]);
  assert.deepEqual(nearest(a, "zzz"), []);
  assert.deepEqual(nearest({ ...a, similarity: [] }, "a"), []);
  assert.equal(barShare(-1.4), 1);
});

test("the note names the measure, short texts and too few texts", () => {
  assert.match(alikeLine(a, "a"), /Cosine Delta over the project's 200 most frequent words/);
  assert.match(alikeLine(a, "a"), /1 text has under 500 words/);
  assert.equal(alikeLine({ ...a, texts: a.texts.slice(0, 2) }, "a"), "Comparing texts needs three texts or more with words.");
});

test("the whole table pastes as a square", () => {
  const lines = alikeTsv(a).trimEnd().split("\n");
  assert.equal(lines[0], "\tFish A\tFish B\tSongs");
  assert.equal(lines[3], "Songs\t-0.600\t-0.500\t1.000");
});
