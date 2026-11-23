import assert from "node:assert/strict";
import { test } from "node:test";
import { ellipsePath, groups, place, plotNote, plotted, ranges, table, ticks, type Point } from "./logic.ts";

const pt = (label: string, f1: number | null, f2: number | null, segment = label): Point => ({ segment, label, startMs: 1000, endMs: 1200, f0: 120, f1, f2, f3: 2500 });

test("points without F1 or F2 do not plot; labels group, empty ones together", () => {
  const ps = plotted([pt("i", 300, 2300), pt(" i ", 320, 2200), pt("a", null, 1300), pt("", 500, 1500)], false);
  assert.deepEqual(ps.map((p) => [p.group, p.x, p.y]), [["i", 2300, 300], ["i", 2200, 320], ["(no label)", 1500, 500]]);
});

test("Lobanov normalization gives z-scores", () => {
  const ps = plotted([pt("i", 300, 2400), pt("a", 700, 1400), pt("u", 500, 900)], true);
  assert.ok(Math.abs(ps[0].y - -1) < 1e-9 && Math.abs(ps[1].y - 1) < 1e-9 && Math.abs(ps[2].y) < 1e-9);
  const xs = ps.map((p) => p.x);
  assert.ok(Math.abs(xs.reduce((a, b) => a + b, 0)) < 1e-9);
});

test("a group's ellipse follows its spread", () => {
  // Spread along x only: the first axis is horizontal, the second is flat.
  const ps = plotted([pt("e", 400, 1900), pt("e", 400, 2000), pt("e", 400, 2100), pt("o", 450, 900)], false);
  const [e, o] = groups(ps);
  assert.equal(e.label, "e");
  assert.equal(e.count, 3);
  assert.equal(e.x, 2000);
  assert.ok(e.ellipse !== null && Math.abs(e.ellipse.rx - 100) < 1e-9 && e.ellipse.ry === 0 && Math.abs(e.ellipse.angle) < 1e-9);
  assert.equal(o.ellipse, null);
});

test("axes run the phonetician's way and widen to hold the data", () => {
  const r = ranges(plotted([pt("i", 150, 3200)], false), false);
  assert.ok(r.y.lo < 150 && r.x.hi > 3200 && r.y.hi === 1000 && r.x.lo === 500);
  const box = { left: 0, top: 0, width: 100, height: 100 };
  // High F2 on the left, low F1 at the top.
  assert.deepEqual(place(r.x.hi, r.y.lo, r, box), { px: 0, py: 0 });
  assert.deepEqual(place(r.x.lo, r.y.hi, r, box), { px: 100, py: 100 });
});

test("ticks are round numbers inside the range", () => {
  assert.deepEqual(ticks({ lo: 200, hi: 1000 }, 4), [200, 400, 600, 800, 1000]);
  assert.deepEqual(ticks({ lo: -2.5, hi: 2.5 }, 5), [-2, -1, 0, 1, 2]);
});

test("the table and the note", () => {
  assert.equal(table([pt("a\tb", 700, null)]), "label\tstart_s\tend_s\tf0_hz\tf1_hz\tf2_hz\tf3_hz\na b\t1.000\t1.200\t120.0\t700\t\t2500");
  assert.equal(plotNote([pt("i", 1, 2), pt("a", null, 2)], 1, 2, "Praat"), "1 of 2 segments plotted, analysed by Praat. 1 had no F1 or F2 (silence or noise). 2 without times left out.");
});

test("an ellipse is traced around its mean", () => {
  const g = { label: "a", count: 3, x: 1000, y: 500, ellipse: { rx: 100, ry: 50, angle: Math.PI / 2 } };
  const path = ellipsePath(g, 4);
  // Turned a quarter: the long axis runs along y.
  assert.ok(Math.abs(path[0].x - 1000) < 1e-9 && Math.abs(path[0].y - 600) < 1e-9);
  assert.ok(Math.abs(path[1].x - 950) < 1e-9 && Math.abs(path[1].y - 500) < 1e-9);
  assert.deepEqual(ellipsePath({ ...g, ellipse: null }), []);
});
