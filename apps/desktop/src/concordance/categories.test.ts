import { test } from "node:test";
import assert from "node:assert/strict";
import { categoriesLine, categoryCounts, lineKey, parseCategories } from "./categories.ts";
import type { Hit } from "./logic.ts";

const hit = (seg: string, at: number): Hit => ({ textId: "t", textTitle: "T", segmentId: seg, tierId: "u", startMs: null, endMs: null, words: ["a", "b", "c"], at, breakdown: null, glosses: null });

test("categories for a search's lines", () => {
  assert.deepEqual(parseCategories(" literal, figurative ,\nunclear, literal,, "), ["literal", "figurative", "unclear"]);
  assert.equal(lineKey(hit("s1", 2)), "s1:2");
  const hits = [hit("s1", 0), hit("s1", 2), hit("s2", 1), hit("s3", 0)];
  const sorted = { categories: ["literal", "figurative"], lines: [["s1:0", "literal"], ["s1:2", "figurative"], ["s2:1", "literal"], ["s9:0", "literal"], ["s3:0", "gone"]] as [string, string][] };
  assert.deepEqual(categoryCounts(hits, sorted), [
    { category: "literal", count: 2 },
    { category: "figurative", count: 1 },
    { category: "", count: 1 },
  ], "a line of another search is not counted; a category no longer named counts as not sorted");
  assert.equal(categoriesLine(hits, sorted), "literal 2 (50%) · figurative 1 (25%) · not sorted yet 1.");
  assert.equal(categoriesLine(hits.slice(0, 3), sorted), "literal 2 (67%) · figurative 1 (33%) · every line sorted.");
  assert.match(categoriesLine(hits, { categories: [], lines: [] }), /^Type the categories/u);
});
