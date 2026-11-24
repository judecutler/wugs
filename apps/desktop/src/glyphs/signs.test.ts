import { test } from "node:test";
import assert from "node:assert/strict";
import { entropy, placeShare, scriptNote, signPairs, signsOf, signsTsv, signTable } from "./signs.ts";

test("a word's signs: letters with their marks, or a transliteration's pieces", () => {
  assert.deepEqual(signsOf("ŋáta.", "letters"), ["ŋ", "á", "t", "a"]);
  assert.deepEqual(signsOf("ko-no-so", "pieces"), ["ko", "no", "so"]);
  assert.deepEqual(signsOf("a.ta=ra", "pieces"), ["a", "ta", "ra"]);
  assert.deepEqual(signsOf("𓂀𓃭", "letters"), ["𓂀", "𓃭"], "hieroglyphs are signs");
});

test("each sign's count and place in the word", () => {
  const rows = signTable([{ word: "ko-no-so", count: 3 }, { word: "ko-wa", count: 1 }, { word: "a", count: 2 }], "pieces");
  const ko = rows.find((r) => r.sign === "ko")!;
  assert.deepEqual([ko.count, ko.words, ko.initial, ko.final], [4, 2, 4, 0]);
  assert.equal(rows.find((r) => r.sign === "a")!.alone, 2);
  assert.equal(rows[0]!.sign, "ko", "commonest first");
  assert.equal(ko.example, "ko-no-so");
  assert.equal(placeShare(ko.initial, ko), "100%");
  assert.equal(placeShare(0, ko), "–");
  assert.match(signsTsv(rows), /^sign\tcount\tword_types/u);
});

test("pairs inside words", () => {
  const pairs = signPairs([{ word: "kata", count: 2 }, { word: "taka", count: 1 }], "letters");
  assert.deepEqual(pairs[0], { first: "k", second: "a", count: 3 }, "kata twice and taka once, the tie broken by the signs");
  assert.ok(pairs.some((p) => p.first === "a" && p.second === "t" && p.count === 2));
});

test("entropy: a fixed order leaves nothing to guess, any order leaves it all", () => {
  // Every word the same four signs in the same order: the next sign is certain.
  const fixed = entropy([{ word: "abcd", count: 10 }], "letters");
  assert.equal(fixed.signs, 4);
  assert.ok(Math.abs(fixed.h1 - 2) < 1e-9, "four equally common signs: 2 bits");
  assert.ok(Math.abs(fixed.conditional) < 1e-9);
  assert.match(scriptNote(fixed), /4 different signs in 40 running signs: as many as an alphabet/u);
  assert.match(scriptNote(fixed), /100% less/u);
  // Every pair of two signs, equally often: the sign before tells little.
  const free = entropy(["aa", "ab", "ba", "bb"].map((word) => ({ word, count: 5 })), "letters");
  assert.ok(free.conditional > 0.9 * free.h1, `${free.conditional} of ${free.h1}`);
  assert.equal(scriptNote(entropy([], "letters")), "");
});
