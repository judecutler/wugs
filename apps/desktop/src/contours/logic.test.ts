import assert from "node:assert/strict";
import { test } from "node:test";
import { contourNote, contourTable, dashOf, lines, means, reference, semitones, valueRange, type ContourPoint } from "./logic.ts";

const pt = (label: string, contour: (number | null)[], i = 0): ContourPoint => ({ segment: `s${i}`, label, startMs: i * 1000, endMs: i * 1000 + 300, contour });
const rise = [100, 105, 110, 115, 120, 125, 130, 135, 140, 145];
const fall = [200, 190, 180, 170, 160, 150, 140, 130, 120, 110];

test("semitones count from the speaker's median", () => {
  assert.equal(reference([pt("a", [100, null, 300]), pt("b", [200])]), 200);
  assert.equal(reference([pt("a", [null])]), null);
  assert.equal(semitones(200, 100), 12);
  const ls = lines([pt("H", [200, null]), pt("L", [100, 100])], "st");
  assert.equal(ls[0]!.values[0], 12);
  assert.equal(ls[0]!.values[1], null);
});

test("each label gets its mean contour, voiced tokens only", () => {
  const ps = [pt("rise", rise, 0), pt("rise", rise.map((v) => v + 10), 1), pt("fall", fall, 2), pt("x", Array(10).fill(null), 3)];
  const ls = lines(ps, "hz");
  assert.equal(ls.length, 3, "a token with no voice is not drawn");
  const ms = means(ls);
  assert.deepEqual(ms.map((m) => [m.label, m.count]), [["rise", 2], ["fall", 1]]);
  assert.equal(ms[0]!.values[0], 105);
  assert.equal(ms[1]!.values[9], 110);
  // A point voiced in one token only takes that token's value.
  const gap = means(lines([pt("a", [100, null]), pt("a", [110, 120])], "hz"));
  assert.deepEqual(gap[0]!.values.slice(0, 2), [105, 120]);
});

test("the range is padded and never flat", () => {
  const r = valueRange(lines([pt("a", [100, 100])], "hz"), "hz");
  assert.ok(r.hi - r.lo >= 20 && r.lo < 100 && r.hi > 100);
  assert.deepEqual(valueRange([], "st"), { lo: -6, hi: 6 });
  assert.equal(dashOf(0), "");
  assert.equal(dashOf(4), "");
});

test("the table and the note", () => {
  const t = contourTable([pt("ma\t1", [100.04, null])]).split("\n");
  assert.ok(t[0]!.startsWith("label\tstart_s\tend_s\tf0_05pct_hz\tf0_15pct_hz"));
  assert.equal(t[1], "ma 1\t0.000\t0.300\t100.0\t");
  assert.equal(contourNote([pt("a", rise), pt("b", [null])], 1, 2, "Praat", "st", 150), "1 of 2 segments drawn, analysed by Praat. 1 had no voiced stretch. 2 without times left out. 0 st is the speaker's median, 150 Hz.");
});
