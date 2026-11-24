import assert from "node:assert/strict";
import { test } from "node:test";
import { asTsv, lemmaQuery, compareCollocates, compareLine, compareTsv, leansTo, sortCollocates, summary, type CollocateRow } from "./logic.ts";

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

test("two words' collocates compared", () => {
  const r = (item: string, logDice: number, together = 3): CollocateRow => ({ item, together, left: 1, right: 2, freq: 9, logLikelihood: 4, mutualInformation: 3, logDice, tScore: 1.5 });
  const strong = [r("tea", 10), r("wind", 9.5), r("Coffee", 8)];
  const powerful = [r("engine", 11), r("wind", 9), r("coffee", 6)];
  const rows = compareCollocates(strong, powerful);
  assert.deepEqual(rows.map((x) => [x.item, x.diff, leansTo(x)]), [
    ["tea", 10, "a"],
    ["Coffee", 2, "a"],
    ["wind", 0.5, "both"],
    ["engine", -11, "b"],
  ], "case-blind match; a missing score counts as 0; within 1 logDice is both");
  const c = (node: number) => ({ rows: [], node, tokens: 100, types: 10 });
  assert.equal(compareLine("strong", "powerful", c(4), c(3), rows), "«strong» (4) against «powerful» (3): 2 collocates go with «strong», 1 with «powerful» and 1 with both alike (within 1 logDice). Those typical of «strong» first.");
  assert.equal(compareLine("strong", "mighty", c(4), c(0), rows), "«mighty» does not occur here.");
  assert.equal(compareTsv("strong", "powerful", rows).split("\n")[4], "engine\t0\t\t3\t11.00\t-11.00");
});

test("a lemma pattern opens as a CQP query", () => {
  assert.equal(lemmaQuery("dog", null), '[lemma="dog" %c]');
  assert.equal(lemmaQuery("walk*", null), '[lemma="walk.*" %c]');
  assert.equal(lemmaQuery("a.b?", { key: "genre", value: "news" }), '[lemma="a\\.b." %c] :: match.text_genre="news"');
});
