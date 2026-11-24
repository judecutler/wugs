import { test } from "node:test";
import assert from "node:assert/strict";
import { nearHits, nearLine, nearTest } from "./near.ts";
import type { Hit } from "./logic.ts";

const hit = (words: string, at: number, len = 1): Hit => ({ textId: "t", textTitle: "T", segmentId: words, tierId: "u", startMs: null, endMs: null, words: words.split(" "), at, len, breakdown: null, glosses: null });

test("a word near the match", () => {
  const hits = [hit("the fox drank water", 1), hit("a fox ran", 1), hit("water, said the old fox", 4), hit("the big red fox", 3, 1)];
  assert.deepEqual(nearHits(hits, "the", 1, "either").map((h) => h.segmentId), ["the fox drank water"]);
  assert.deepEqual(nearHits(hits, "water", 2, "after").map((h) => h.segmentId), ["the fox drank water"]);
  assert.deepEqual(nearHits(hits, "water", 5, "before").map((h) => h.segmentId), ["water, said the old fox"], "punctuation at a word's edge ignored");
  assert.deepEqual(nearHits(hits, "THE", 3, "before").map((h) => h.segmentId), ["the fox drank water", "water, said the old fox", "the big red fox"], "case ignored");
  assert.deepEqual(nearHits(hits, "r*", 3, "either").map((h) => h.segmentId), ["a fox ran", "the big red fox"]);
  assert.deepEqual(nearHits(hits, "", 3, "either"), [], "nothing typed keeps nothing");
  assert.deepEqual(nearHits(hits, "*", 3, "either"), [], "a lone * would keep every line with any neighbour: refused");
  // A phrase match: the window starts after its last word, and the match itself never counts.
  const phrase = hit("she said that he said no", 1, 2);
  assert.equal(nearHits([phrase], "no", 2, "after").length, 0);
  assert.equal(nearHits([phrase], "no", 3, "after").length, 1);
  assert.equal(nearHits([phrase], "that", 3, "before").length, 0);
});

test("marks are kept", () => {
  assert.equal(nearTest("ka")("ká"), false);
  assert.equal(nearTest("k?")("ká"), true);
  assert.equal(nearTest("é")("é"), true, "NFC");
});

test("the line", () => {
  assert.equal(nearLine(3, 10, " the ", 3, "either"), " 3 lines of 10 have «the» within 3 words before or after.");
  assert.equal(nearLine(1, 10, "the", 1, "before"), " 1 line of 10 has «the» right before it.");
  assert.equal(nearLine(4, 4, "the", 2, "after"), " Every line has «the» within 2 words after.");
});
