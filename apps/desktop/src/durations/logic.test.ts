import assert from "node:assert/strict";
import { test } from "node:test";
import { atLeast, cannotCompare, compare, compareLine, defaultPair, durationGroups, durationsNote, incompleteBeta, msTicks, summaryTsv, tokensTsv, tTestP } from "./logic.ts";

const seg = (id: string, start: number | null, end: number | null, content: string, tier = "v") => ({ id, tier, startMs: start, endMs: end, content });

test("segments grouped by label, shortest mean first", () => {
  const { groups, untimed } = durationGroups(
    [
      seg("1", 0, 80, "a"),
      seg("2", 100, 200, "a "),
      seg("3", 300, 480, "aː"),
      seg("4", 500, 700, "aː"),
      seg("5", 800, 1020, "aː"),
      seg("6", null, null, "a"),
      seg("7", 1100, 1200, ""),
      seg("8", 0, 50, "a", "other"),
      seg("9", 1300, 1300, "i"),
    ],
    "v",
  );
  assert.equal(untimed, 2, "no times, or none that end after they start");
  assert.deepEqual(groups.map((g) => [g.label, g.n, g.mean, g.median, g.min, g.max]), [
    ["a", 2, 90, 90, 80, 100],
    ["aː", 3, 200, 200, 180, 220],
  ]);
  assert.ok(Math.abs(groups[0]!.sd! - Math.sqrt(200)) < 1e-9);
  assert.deepEqual(defaultPair(groups), ["a", "aː"]);
  assert.deepEqual(atLeast(groups, 3).map((g) => g.label), ["aː"]);
});

test("the t distribution matches tables", () => {
  // Two-sided critical values: t(10) = 2.228 at .05, t(30) = 2.750 at .01.
  assert.ok(Math.abs(tTestP(2.228, 10) - 0.05) < 5e-4);
  assert.ok(Math.abs(tTestP(2.75, 30) - 0.01) < 5e-4);
  assert.ok(Math.abs(tTestP(0, 5) - 1) < 1e-12);
  assert.ok(Math.abs(incompleteBeta(0.5, 2, 2) - 0.5) < 1e-12);
  assert.ok(Math.abs(incompleteBeta(0.3, 1, 1) - 0.3) < 1e-12);
});

test("Welch's comparison and its words", () => {
  const { groups } = durationGroups([seg("1", 0, 80, "a"), seg("2", 100, 200, "a"), seg("3", 300, 480, "aː"), seg("4", 500, 700, "aː"), seg("5", 800, 1020, "aː")], "v");
  const c = compare(groups[0]!, groups[1]!)!;
  // va = 200/2 = 100, vb = 400/3; t = 110 / sqrt(100 + 133.3); df by Welch–Satterthwaite.
  const va = 100;
  const vb = 400 / 3;
  assert.ok(Math.abs(c.t - 110 / Math.sqrt(va + vb)) < 1e-9);
  assert.ok(Math.abs(c.df - (va + vb) ** 2 / (va ** 2 / 1 + vb ** 2 / 2)) < 1e-9);
  assert.ok(Math.abs(c.ratio - 200 / 90) < 1e-9);
  assert.match(compareLine(c), /^«aː» is 2\.22 times as long as «a» on average \(200 ms against 90 ms\): Welch's t\(2\.\d\) = 7\.2\d, p = 0\.0\d+, a difference unlikely to be chance\.$/u);
  const one = durationGroups([seg("1", 0, 80, "a"), seg("3", 300, 480, "aː"), seg("4", 500, 700, "aː")], "v").groups;
  assert.equal(compare(one[0]!, one[1]!), null);
  assert.match(cannotCompare(one[0], one[1]), /«a» has one token only/u);
  assert.match(cannotCompare(one[1], one[1]), /two different/u);
});

test("notes and tables", () => {
  const { groups } = durationGroups([seg("1", 0, 80, "a"), seg("2", 100, 200, "a"), seg("3", 300, 480, "aː")], "v");
  assert.match(durationsNote(groups, 1, 1), /^3 segments under 2 labels\. 1 label is left out of the plot for having too few tokens\. 1 labelled segment has no times and is left out\./u);
  assert.match(durationsNote([], 0, 0), /No segment/u);
  assert.equal(summaryTsv(groups).split("\n")[1], "a\t2\t90\t14\t90\t80\t100");
  assert.equal(summaryTsv(groups).split("\n")[2], "aː\t1\t180\t\t180\t180\t180");
  assert.equal(tokensTsv(groups).split("\n")[3], "aː\t3\t0.300\t0.480\t180");
  assert.deepEqual(msTicks(220), [0, 50, 100, 150, 200]);
  assert.deepEqual(msTicks(1900), [0, 500, 1000, 1500]);
});
