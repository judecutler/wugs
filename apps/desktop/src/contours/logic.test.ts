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

test("a speaker's pitch range and each label's level", async () => {
  const { pitchRange, rangeLine, levels, quantile } = await import("./logic.ts");
  assert.equal(quantile([1, 2, 3, 4], 0.5), 2.5);
  assert.equal(quantile([10, 20], 0.1), 11);
  const tok = (i: number, label: string, hz: number) => ({ segment: `s${i}`, label, startMs: 0, endMs: 100, contour: Array.from({ length: 10 }, (_, k) => (k === 9 ? null : hz)) });
  // Three H tokens at 200 Hz, three L at 100 Hz: an octave apart.
  const ps = [tok(1, "H", 200), tok(2, "H", 200), tok(3, "H", 200), tok(4, "L", 100), tok(5, "L", 100), tok(6, "L", 100)];
  const r = pitchRange(ps)!;
  assert.equal(r.points, 54);
  assert.ok(Math.abs(r.median - 150) < 1e-9);
  assert.equal(r.p10, 100);
  assert.equal(r.p90, 200);
  assert.ok(Math.abs(r.spanSt - 12) < 1e-9);
  assert.match(rangeLine("A", r), /^A's pitch: median 150 Hz; 80% of the 54 voiced points lie between 100 Hz and 200 Hz, a span of 12\.0 semitones/);
  assert.equal(pitchRange(ps.slice(0, 2)), null);
  assert.match(rangeLine("A", null), /Too few voiced points/);
  const lv = levels(ps);
  assert.deepEqual(lv.map((l) => [l.label, l.tokens, l.median]), [["H", 3, 200], ["L", 3, 100]]);
  assert.ok(Math.abs(lv[0]!.st - 12 * Math.log2(200 / 150)) < 1e-9);
});
