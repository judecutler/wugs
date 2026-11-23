import { test } from "node:test";
import assert from "node:assert/strict";
import { marks, per1000, plotTsv, sortPlots, spreadWords, type Dispersion } from "./plot.ts";

test("marks fold positions into columns, one each", () => {
  assert.deepEqual(marks([0, 1, 50, 99], 100, 10), [0, 5, 9]);
  assert.deepEqual(marks([3], 0, 10), []);
  // The last word lands in the last column, never past it.
  assert.deepEqual(marks([9], 10, 4), [3]);
});

test("spread in words", () => {
  const d: Dispersion = {
    texts: [
      { textId: "a", title: "A", words: 100, hits: 4, positions: [1, 2, 3, 4] },
      { textId: "b", title: "B", words: 300, hits: 0, positions: [] },
    ],
    words: 400,
    hits: 4,
    dp: 0.75,
    dpNorm: 1,
  };
  assert.match(spreadWords(d), /^Found 4 times in 1 of 2 texts \(400 running words\)\. DP 0\.75 \(normalised 1\.00\): almost all in one text\./);
  assert.equal(spreadWords({ ...d, hits: 0, dp: null, dpNorm: null }), "Not found.");
  assert.match(spreadWords({ ...d, texts: [d.texts[0]!], dp: null, dpNorm: null }), /Spread needs two texts or more\.$/);
  assert.equal(per1000(4, 100), "40.0");
});

test("sorting keeps text order or ranks by matches and rate, empty texts dropped", () => {
  const t = (id: string, words: number, hits: number) => ({ textId: id, title: id, words, hits, positions: [] });
  const texts = [t("a", 100, 1), t("b", 10, 1), t("c", 0, 0), t("d", 100, 3)];
  assert.deepEqual(sortPlots(texts, "text").map((x) => x.textId), ["a", "b", "d"]);
  assert.deepEqual(sortPlots(texts, "hits").map((x) => x.textId), ["d", "b", "a"]);
  assert.deepEqual(sortPlots(texts, "rate").map((x) => x.textId), ["b", "d", "a"]);
});

test("the table has a header and a row per text", () => {
  const d: Dispersion = { texts: [{ textId: "a", title: "A\tB", words: 10, hits: 2, positions: [1, 5] }], words: 10, hits: 2, dp: null, dpNorm: null };
  assert.equal(plotTsv(d), "Text\tWords\tMatches\tPer 1000\tWord positions\nA B\t10\t2\t200.0\t1 5\n");
});
