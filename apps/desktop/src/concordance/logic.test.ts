import assert from "node:assert/strict";
import { test } from "node:test";
import { asTsv, clip, countLine, hasTags, isRtl, withTags, keyword, left, right, sortHits, type Hit } from "./logic.ts";

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

test("a match of several words is the keyword whole, and the right context starts after it", () => {
  const h: Hit = { ...hit(["went", "to", "the", "park", "."], 1), len: 3 };
  assert.equal(keyword(h), "to the park");
  assert.equal(left(h), "went");
  assert.equal(right(h), ".");
  const other: Hit = { ...hit(["x", "to", "a", "b", "Alpha"], 1), len: 2 };
  // Sorting by the word after reads past the whole match.
  assert.deepEqual(sortHits([h, other], "right").map((x) => right(x)), [". ", "b Alpha"].map((s) => s.trim()));
});

test("the CQP chip is the Concordance's own; Frequencies keeps the four kinds", async () => {
  const { KINDS, PANEL_KINDS } = await import("./logic.ts");
  assert.equal(KINDS.length, 4);
  assert.equal(PANEL_KINDS.at(-1)?.id, "query");
});

test("a tagged line reads word_TAG", () => {
  const h = { textId: "t", textTitle: "T", segmentId: "s", tierId: "u", startMs: null, endMs: null, words: ["a", "el", "mar", "."], at: 0, len: 3, breakdown: null, glosses: null, tags: ["ADP", "DET", "NOUN", ""] };
  assert.equal(hasTags([h]), true);
  assert.equal(hasTags([{ ...h, tags: undefined }]), false);
  assert.equal(hasTags([{ ...h, tags: ["", "", "", ""] }]), false);
  const t = withTags(h);
  assert.equal(keyword(t), "a_ADP el_DET mar_NOUN");
  assert.equal(right(t), ".");
  assert.equal(withTags({ ...h, tags: undefined }).words[0], "a");
});

test("second-word and match sorts, as AntConc's sort levels", () => {
  const hits = [hit(["x", "b", "KEY", "q", "z"], 2), hit(["y", "a", "KEY", "p", "a"], 2), hit(["w", "c", "KEY"], 2)];
  // 2L first: w, x, y; ties broken by 1L.
  assert.deepEqual(sortHits(hits, "left2").map((h) => h.words[0]), ["w", "x", "y"]);
  // 2R first: the line with no second word after sorts first, then a, z.
  assert.deepEqual(sortHits(hits, "right2").map((h) => h.words[0]), ["w", "y", "x"]);
  // The match itself, then the word after: different matches of a pattern sort together.
  const glob = [hit(["kanu", "z"], 0), hit(["kani", "b"], 0), hit(["kani", "a"], 0)];
  assert.deepEqual(sortHits(glob, "keyword").map((h) => h.words.join(" ")), ["kani a", "kani b", "kanu z"]);
  // A phrase match counts its whole length before the word after.
  const phrase = { ...hit(["a", "b", "c", "d"], 1), len: 2 };
  assert.deepEqual(sortHits([phrase], "right2"), [phrase]);
});

test("the sorted word is marked in its context", async () => {
  const { markWord, sortedWord } = await import("./logic.ts");
  assert.deepEqual(markWord("a b c", "left", -1), ["a b ", "c", ""]);
  assert.deepEqual(markWord("a b c", "left", -2), ["a ", "b", " c"]);
  assert.deepEqual(markWord("x y", "right", 1), ["", "x", " y"]);
  assert.deepEqual(markWord("x y", "right", 2), ["x ", "y", ""]);
  assert.deepEqual(markWord("x", "right", 2), ["x", "", ""], "too short");
  assert.deepEqual(markWord("", "left", -1), ["", "", ""]);
  assert.deepEqual(markWord("…far b", "left", -2), ["…far b", "", ""], "a clipped word is not marked");
  assert.equal(sortedWord("left2"), -2);
  assert.equal(sortedWord("right"), 1);
  assert.equal(sortedWord("keyword"), null);
  assert.equal(sortedWord("text"), null);
});
