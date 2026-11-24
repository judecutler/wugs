import { test } from "node:test";
import assert from "node:assert/strict";
import { categoriesLine, categoryCounts, chiSquareP, crossTab, crossTabTsv, independence, independenceLine, lineKey, parseCategories } from "./categories.ts";
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

test("chi-square tails match the tables", () => {
  // df 1 at 3.841 and df 4 at 9.488 are the 5% points; df 2 at 5.991.
  assert.ok(Math.abs(chiSquareP(3.841, 1) - 0.05) < 0.0005);
  assert.ok(Math.abs(chiSquareP(9.488, 4) - 0.05) < 0.0005);
  assert.ok(Math.abs(chiSquareP(5.991, 2) - 0.05) < 0.0005);
  assert.ok(Math.abs(chiSquareP(1, 10) - 0.99982829) < 1e-6);
  assert.equal(chiSquareP(0, 3), 1);
});

test("sorted lines are cross-tabulated by speaker and tested", () => {
  const line = (segmentId: string, textTitle: string) => ({ segmentId, at: 0, textTitle }) as unknown as Hit;
  const hits = [line("a", "Ana"), line("b", "Ana"), line("c", "Ana"), line("d", "Bo"), line("e", "Bo"), line("f", "Bo"), line("g", "Bo")];
  const sorted = { categories: ["ng", "n"], lines: [["a:0", "ng"], ["b:0", "ng"], ["c:0", "n"], ["d:0", "n"], ["e:0", "n"], ["f:0", "n"]] as [string, string][] };
  const t = crossTab(hits, sorted, (h) => h.textTitle);
  assert.deepEqual(t, { groups: ["Ana", "Bo"], categories: ["ng", "n"], cells: [[2, 1], [0, 3]] });
  assert.equal(crossTabTsv(t, "speaker"), "speaker\tng\tn\tlines\nAna\t2\t1\t3\nBo\t0\t3\t3\n");
  const r = independence(t.cells)!;
  assert.equal(r.df, 1);
  assert.ok(Math.abs(r.chi2 - 3) < 1e-9);
  assert.ok(Math.abs(r.v - Math.sqrt(0.5)) < 1e-9);
  assert.match(independenceLine(r, "speaker"), /^The categories do not differ by speaker beyond chance \(χ² = 3\.00, df = 1, p = 0\.083; Cramér's V = 0\.71\)\. Many cells/);
  assert.equal(independence([[3, 0]]), null);
  assert.match(independenceLine(null, "genre"), /at least two genre values/);
  // A clear difference.
  const big = independence([[40, 10], [10, 40]])!;
  assert.ok(big.p < 0.001);
  assert.match(independenceLine(big, "genre"), /^The categories differ by genre more than chance would have it \(χ² = 36\.00, df = 1, p < 0\.001; Cramér's V = 0\.60\)\.$/);
});
