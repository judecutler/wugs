import assert from "node:assert/strict";
import { test } from "node:test";
import { asTsv, sortCollocates, summary, type CollocateRow } from "./logic.ts";

const row = (item: string, together: number, ll: number, mi: number): CollocateRow => ({ item, together, left: together, right: 0, freq: together * 2, logLikelihood: ll, mutualInformation: mi, logDice: mi + 8, tScore: together / 2 });
const rows = [row("old", 3, 12.5, 3.1), row("a", 1, 0.4, 6.0), row("the", 3, 5.0, 1.0)];

test("sorts by each measure, alphabet ignoring case", () => {
  assert.deepEqual(sortCollocates(rows, "ll").map((r) => r.item), ["old", "a", "the"], "backend order kept");
  assert.deepEqual(sortCollocates(rows, "mi").map((r) => r.item), ["a", "old", "the"]);
  assert.deepEqual(sortCollocates(rows, "dice").map((r) => r.item), ["a", "old", "the"]);
  assert.deepEqual(sortCollocates(rows, "t").map((r) => r.item), ["old", "the", "a"]);
  assert.deepEqual(sortCollocates(rows, "together").map((r) => r.item), ["old", "the", "a"], "ties by log-likelihood");
  assert.deepEqual(sortCollocates([row("Bee", 1, 0, 0), row("ant", 1, 0, 0)], "alpha").map((r) => r.item), ["ant", "Bee"]);
});

test("summary says what was counted, in words", () => {
  assert.equal(summary("fox", { rows: [], node: 0, tokens: 0, types: 0 }, 5, 5), "No words yet.");
  assert.equal(summary("fox", { rows: [], node: 0, tokens: 9, types: 0 }, 5, 5), "«fox» does not occur here.");
  assert.equal(summary("fox", { rows, node: 1, tokens: 1200, types: 3 }, 1, 1), "«fox» occurs 1 time in 1,200 words; 3 different words keep it company within 1 word either side.");
  assert.match(summary("fox", { rows, node: 4, tokens: 20, types: 9 }, 2, 3), /within 2 before and 3 after\. 3 are listed: those met together often enough/);
});

test("copies as a table", () => {
  assert.equal(asTsv([row("o\tld", 3, 12.5, 3.14159)]).split("\n")[1], "o ld\t3\t3\t0\t6\t12.50\t3.14\t11.14\t1.50");
});
