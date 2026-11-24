import { test } from "node:test";
import assert from "node:assert/strict";
import { byPeriod, marks, per1000, periodTsv, plotTsv, sortPlots, spreadWords, timeKeys, yearOf, type Dispersion, type TextPlot } from "./plot.ts";

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

test("matches summed by a text detail", async () => {
  const { byDetail, detailTsv, NOT_GIVEN, plotDetails } = await import("./plot.ts");
  const t = (id: string, words: number, hits: number, meta: [string, string][]) => ({ textId: id, title: id, words, hits, positions: [], meta });
  const texts = [t("a", 1000, 10, [["genre", "narrative"], ["speaker", "A"]]), t("b", 1000, 2, [["genre", "conversation"]]), t("c", 500, 5, [["genre", "narrative"]]), t("d", 200, 9, [])];
  assert.deepEqual(plotDetails(texts), ["genre", "speaker"]);
  const rows = byDetail(texts, "genre");
  assert.deepEqual(rows.map((r) => [r.value, r.texts, r.words, r.hits]), [["narrative", 2, 1500, 15], ["conversation", 1, 1000, 2], [NOT_GIVEN, 1, 200, 9]]);
  assert.equal(detailTsv(rows, "genre").split("\n")[1], "narrative\t2\t1500\t15\t10.0");
});

test("a detail's year is read from dates and years as people write them", () => {
  assert.equal(yearOf("1953-04-02"), 1953);
  assert.equal(yearOf("c. 850"), 850);
  assert.equal(yearOf("450 BCE"), -450);
  assert.equal(yearOf("folio 12"), null);
  assert.equal(yearOf("2.5"), null);
});

test("matches are summed over time, gaps kept", () => {
  const t = (id: string, date: string | null, words: number, hits: number): TextPlot => ({ textId: id, title: id, words, hits, positions: [], meta: date === null ? [] : [["date", date]] });
  const texts = [t("a", "1950-01-01", 1000, 4), t("b", "1953", 500, 1), t("c", "1990", 1000, 0), t("d", null, 100, 9), t("e", "notes", 50, 0)];
  assert.deepEqual(timeKeys(texts), ["date"]);
  const p = byPeriod(texts, "date");
  assert.equal(p.width, 10);
  assert.deepEqual(p.rows.map((r) => [r.label, r.texts, r.hits]), [["1950s", 2, 5], ["1960s", 0, 0], ["1970s", 0, 0], ["1980s", 0, 0], ["1990s", 1, 0]]);
  assert.equal(p.undated, 2);
  assert.equal(periodTsv(p, "date").split("\n")[1], "1950s\t1950\t1959\t2\t1500\t5\t3.3");
  assert.equal(byPeriod([t("x", "2001", 10, 1), t("y", "2003", 10, 1)], "date").rows.map((r) => r.label).join(" "), "2001 2002 2003");
});
