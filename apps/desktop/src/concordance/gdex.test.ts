import assert from "node:assert/strict";
import { test } from "node:test";
import { bestFirst, freqKey, gdex, gdexTitle } from "./gdex.ts";
import { sortHits, sortedWord, type Hit } from "./logic.ts";

const hit = (words: string, at: number, id = words): Hit => ({ textId: "t", textTitle: "T", segmentId: id, tierId: "u", startMs: null, endMs: null, words: words.split(" "), at, breakdown: null, glosses: null });
const counts = new Map([["the", 50], ["dog", 10], ["saw", 8], ["a", 40], ["cat", 9], ["in", 30], ["garden", 5], ["yesterday", 4], ["xylograph", 1]]);
const uses = (k: string) => counts.get(k);

test("a readable sentence of common words scores best", () => {
  const good = gdex(hit("the dog saw a cat in the garden", 1), uses, true);
  assert.equal(good.score, 1);
  assert.deepEqual(good.why, []);
  const short = gdex(hit("the dog", 1), uses, true);
  assert.ok(Math.abs(short.score - 0.4 * 0.9) < 1e-9);
  assert.deepEqual(short.why, ["short (2 words)", "ends the sentence"]);
  const rare = gdex(hit("the dog saw a xylograph in the garden", 1), uses, true);
  assert.ok(rare.score < 1 && rare.why[0] === "1 rare word (xylograph)");
  const odd = gdex(hit("the dog saw 3 cats in the garden", 1), uses, null);
  assert.ok(odd.why.includes("1 number or symbol"));
  assert.equal(gdex(hit("the dog saw a cat in the garden", 1), uses, false).score, 0.7);
  // Case and composition do not change a word's count.
  assert.equal(freqKey("The"), "the");
  assert.equal(freqKey("é"), "é");
  assert.match(gdexTitle(short), /^As a dictionary example: 36 of 100; short \(2 words\); ends the sentence\.$/);
  assert.equal(gdexTitle(good), "As a dictionary example: 100 of 100.");
});

test("best examples first, ties in text order; the plain sort leaves them as they are", () => {
  const lines = [hit("dog", 0, "a"), hit("the dog saw a cat in the garden yesterday", 1, "b"), hit("the dog saw a xylograph", 1, "c"), hit("a dog saw the cat in a garden", 1, "d")];
  assert.deepEqual(bestFirst(lines, uses, () => null).map((h) => h.segmentId), ["b", "d", "c", "a"]);
  assert.deepEqual(sortHits(lines, "best").map((h) => h.segmentId), ["a", "b", "c", "d"]);
  assert.equal(sortedWord("best"), null);
});
