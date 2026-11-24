import { test } from "node:test";
import assert from "node:assert/strict";
import { fromTo, sparkPoints, steadiness, trendsLine, trendsTsv, type Trends } from "./trends.ts";

const t: Trends = {
  keys: ["date"],
  key: "date",
  labels: ["1950s", "1960s", "1970s"],
  periodWords: [100, 0, 200],
  undated: 1,
  tested: 12,
  rising: [{ item: "okay", total: 9, rates: [0, null, 40], tau: 1 }],
  falling: [],
};

test("a rate line skips empty periods and scales to its top", () => {
  assert.equal(sparkPoints([0, null, 40], 60, 20), "0.0,20.0 60.0,0.0");
  assert.equal(sparkPoints([null, 3], 60, 20), null);
});

test("trends are said in words and copied with a column per period", () => {
  assert.equal(steadiness(-0.95), "every step");
  assert.equal(steadiness(0.6), "mostly");
  assert.equal(fromTo(t.rising[0]!), "0.0 → 40");
  assert.equal(trendsLine(t, 10), "The texts with a year in «date» fall in 2 periods; trends need words in three or more.");
  assert.equal(trendsLine({ ...t, periodWords: [1, 1, 1] }, 10), "12 words used 10 times or more, over 3 periods (1950s to 1970s): 1 rising and 0 falling steadily, by their rate per 1000 words. 1 text has no year in «date» and is left out.");
  assert.match(trendsLine({ ...t, key: "" }, 10), /^No detail gives a year/);
  assert.equal(trendsTsv(t), "direction\tword\tuses\ttau\t1950s\t1960s\t1970s\nrising\tokay\t9\t1.00\t0.00\t\t40.00\n");
});
