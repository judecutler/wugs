import assert from "node:assert/strict";
import { test } from "node:test";
import { accept, addWord, complete, completionIndex, wordAt } from "./complete.ts";

const index = () => completionIndex(new Map([["kanima", 9], ["kani", 3], ["kanira", 4], ["kesi", 1], ["ŋai", 5]]));

test("the commonest longer word the typed start begins", () => {
  assert.deepEqual(complete(index(), "ka"), { word: "kanima", count: 9 });
  assert.deepEqual(complete(index(), "kanir"), { word: "kanira", count: 4 });
  assert.equal(complete(index(), "kanima"), null, "a whole word is not completed");
  assert.equal(complete(index(), "k"), null, "one letter is too short");
  assert.equal(complete(index(), "ke"), null, "a word used once is not offered");
  assert.deepEqual(complete(index(), "ŊA"), { word: "ŋai", count: 5 }, "case aside");
});

test("the word at the caret", () => {
  assert.deepEqual(wordAt("nikani ka", 9), { start: 7, word: "ka" });
  assert.deepEqual(wordAt("nikani ", 7), { start: 7, word: "" });
  assert.deepEqual(wordAt("«ka", 3), { start: 1, word: "ka" });
});

test("accepting keeps the typed capitals and puts the caret after the word", () => {
  assert.deepEqual(accept("nikani Ka toru", 9, "kanima"), { text: "nikani Kanima toru", caret: 13 });
});

test("a new word joins the index in order", () => {
  const i = index();
  addWord(i, "Kanu");
  assert.ok(i.words.indexOf("kanu") > i.words.indexOf("kanira"));
  i.counts.set("kanu", 50);
  assert.equal(complete(i, "kan")?.word, "kanu");
});
