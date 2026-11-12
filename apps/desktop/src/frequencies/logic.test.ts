import { test } from "node:test";
import assert from "node:assert/strict";
import { asTsv, filterRows, perThousand, ranked, sortRows, summary } from "./logic.ts";

const rows = [
  { item: "kesi", count: 5, texts: 1 },
  { item: "ŋai", count: 3, texts: 3 },
  { item: "Ata", count: 3, texts: 2 },
];

test("sorts by count (as given), alphabetically or by spread", () => {
  assert.deepEqual(sortRows(rows, "count").map((r) => r.item), ["kesi", "ŋai", "Ata"]);
  assert.deepEqual(sortRows(rows, "alpha").map((r) => r.item), ["Ata", "kesi", "ŋai"]);
  assert.deepEqual(sortRows(rows, "texts").map((r) => r.item), ["ŋai", "Ata", "kesi"]);
});

test("per thousand keeps one decimal only for small rates", () => {
  assert.equal(perThousand(3, 1000), "3.0");
  assert.equal(perThousand(25, 1000), "25");
  assert.equal(perThousand(1, 0), "0");
});

test("filter ignores case, not diacritics", () => {
  assert.deepEqual(filterRows(rows, "AT").map((r) => r.item), ["Ata"]);
  assert.deepEqual(filterRows(rows, "na").map((r) => r.item), []);
  assert.equal(filterRows(rows, " ").length, 3);
});

test("summary and table", () => {
  const noun = { one: "word", many: "words" };
  assert.equal(summary({ rows: [], tokens: 0, types: 0 }, noun), "No words yet.");
  assert.equal(summary({ rows, tokens: 11, types: 3 }, noun), "11 words, 3 different.");
  assert.match(summary({ rows, tokens: 1200, types: 9000 }, noun), /The 3 most frequent are listed/);
  assert.equal(asTsv(ranked(rows).slice(0, 1), 10).split("\n")[1], "1\tkesi\t5\t500\t1");
});

test("a row keeps its frequency rank through sorting and filtering", () => {
  const r = sortRows(filterRows(ranked(rows), "a"), "alpha");
  assert.deepEqual(r.map((x) => [x.item, x.rank]), [["Ata", 3], ["ŋai", 2]]);
  assert.equal(asTsv(r, 11).split("\n")[1]!.split("\t")[0], "3");
});
