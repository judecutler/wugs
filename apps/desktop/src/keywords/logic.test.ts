import assert from "node:assert/strict";
import { test } from "node:test";
import { asTsv, keywordRows, significant, summary, type KeywordRow } from "./logic.ts";

const row = (item: string, ll: number, lr: number): KeywordRow => ({ item, focus: 3, reference: 1, logLikelihood: ll, logRatio: lr, percentDiff: 50 });
const rows = [row("fox", 12, 4), row("the", 2, 0.5), row("kesi", -9, -3), row("ŋai", -1, -5)];

test("splits by side and sorts", () => {
  assert.deepEqual(keywordRows(rows, "more", "ll").map((r) => r.item), ["fox", "the"]);
  assert.deepEqual(keywordRows(rows, "less", "ll").map((r) => r.item), ["kesi", "ŋai"], "most negative first");
  assert.deepEqual(keywordRows(rows, "less", "ratio").map((r) => r.item), ["ŋai", "kesi"]);
  assert.equal(significant(rows[0]!), true);
  assert.equal(significant(rows[1]!), false);
});

test("summary and table", () => {
  assert.match(summary("Fox", { rows, focusTokens: 8, referenceTokens: 0, types: 4 }), /nothing to compare/);
  assert.match(summary("Fox", { rows, focusTokens: 8, referenceTokens: 1200, types: 4 }), /«Fox» \(8 words\) against .* \(1,200 words\)/);
  assert.equal(asTsv([row("fox", 12, 4)]).split("\n")[1], "fox\t3\t1\t12.00\t4.00\t50.0");
});
