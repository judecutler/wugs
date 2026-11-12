import assert from "node:assert/strict";
import { test } from "node:test";
import { asTsv, clip, countLine, isRtl, keyword, left, right, sortHits, type Hit } from "./logic.ts";

const hit = (words: string[], at: number, title = "T"): Hit => ({ textId: title, textTitle: title, segmentId: words.join("_"), tierId: "u", startMs: 0, endMs: 1, words, at, breakdown: null, glosses: null });

test("keyword in the middle with its context either side", () => {
  const h = hit(["ni", "kani", "ŋai"], 1);
  assert.equal(left(h), "ni");
  assert.equal(keyword(h), "kani");
  assert.equal(right(h), "ŋai");
});

test("sorting by the word before compares the nearest word first", () => {
  const hits = [hit(["b", "z", "KEY"], 2), hit(["c", "a", "KEY"], 2), hit(["KEY"], 0)];
  assert.deepEqual(sortHits(hits, "left").map((h) => h.words.join(" ")), ["KEY", "c a KEY", "b z KEY"]);
  assert.deepEqual(sortHits(hits, "text"), hits);
});

test("sorting by the word after ignores case but not diacritics", () => {
  const hits = [hit(["KEY", "Zeta"], 0), hit(["KEY", "abd"], 0), hit(["KEY", "Abc"], 0)];
  assert.deepEqual(sortHits(hits, "right").map((h) => right(h)), ["Abc", "abd", "Zeta"]);
  // na and ŋa are not ties: their order is stable and not by input order alone.
  const a = sortHits([hit(["KEY", "ŋa"], 0), hit(["KEY", "na"], 0)], "right").map((h) => right(h));
  const b = sortHits([hit(["KEY", "na"], 0), hit(["KEY", "ŋa"], 0)], "right").map((h) => right(h));
  assert.deepEqual(a, b);
});

test("right-to-left scripts are recognised", () => {
  assert.ok(isRtl("كتاب"));
  assert.ok(isRtl("ספר"));
  assert.ok(!isRtl("ŋai"));
});

test("long context is cut at a word, with … on the far side", () => {
  assert.equal(clip("one two three four", 9, "left"), "… four");
  assert.equal(clip("one two three four", 9, "right"), "one two …");
  assert.equal(clip("short", 9, "left"), "short");
});

test("the count line says what is shown", () => {
  assert.equal(countLine(0, 0, 0), "Nothing found.");
  assert.equal(countLine(1, 1, 1), "1 occurrence in 1 text.");
  assert.equal(countLine(2000, 2500, 4), "2500 found in 4 texts; the first 2000 are shown.");
});

test("tab-separated rows keep tabs out of the cells", () => {
  assert.equal(asTsv([hit(["a\tb", "K", "c"], 1, "Title")]), "left\tkeyword\tright\ttext\na b\tK\tc\tTitle");
});
