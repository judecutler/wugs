import assert from "node:assert/strict";
import { test } from "node:test";
import { bark, ellipsePath, groups, movement, parseMethod, place, plotNote, plotted, ranges, table, ticks, trajectories, wattCentroid, type Point } from "./logic.ts";

const pt = (label: string, f1: number | null, f2: number | null, segment = label): Point => ({ segment, label, startMs: 1000, endMs: 1200, f0: 120, f1, f2, f3: 2500 });

test("points without F1 or F2 do not plot; labels group, empty ones together", () => {
  const ps = plotted([pt("i", 300, 2300), pt(" i ", 320, 2200), pt("a", null, 1300), pt("", 500, 1500)], "hz");
  assert.deepEqual(ps.map((p) => [p.group, p.x, p.y]), [["i", 2300, 300], ["i", 2200, 320], ["(no label)", 1500, 500]]);
});

test("Lobanov normalization gives z-scores", () => {
  const ps = plotted([pt("i", 300, 2400), pt("a", 700, 1400), pt("u", 500, 900)], "lobanov");
  assert.ok(Math.abs(ps[0].y - -1) < 1e-9 && Math.abs(ps[1].y - 1) < 1e-9 && Math.abs(ps[2].y) < 1e-9);
  const xs = ps.map((p) => p.x);
  assert.ok(Math.abs(xs.reduce((a, b) => a + b, 0)) < 1e-9);
});

test("a group's ellipse follows its spread", () => {
  // Spread along x only: the first axis is horizontal, the second is flat.
  const ps = plotted([pt("e", 400, 1900), pt("e", 400, 2000), pt("e", 400, 2100), pt("o", 450, 900)], "hz");
  const [e, o] = groups(ps);
  assert.equal(e.label, "e");
  assert.equal(e.count, 3);
  assert.equal(e.x, 2000);
  assert.ok(e.ellipse !== null && Math.abs(e.ellipse.rx - 100) < 1e-9 && e.ellipse.ry === 0 && Math.abs(e.ellipse.angle) < 1e-9);
  assert.equal(o.ellipse, null);
});

test("axes run the phonetician's way and widen to hold the data", () => {
  const r = ranges(plotted([pt("i", 150, 3200)], "hz"), "hz");
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
  assert.equal(table([pt("a\tb", 700, null)]), "label\tstart_s\tend_s\tf0_hz\tf1_hz\tf2_hz\tf3_hz\tf1_20pct_hz\tf2_20pct_hz\tf1_80pct_hz\tf2_80pct_hz\na b\t1.000\t1.200\t120.0\t700\t\t2500\t\t\t\t");
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

test("a label's trajectory runs from its mean 20% point to its mean 80% point", () => {
  const ai = (f1a: number, f2a: number, f1b: number, f2b: number): Point => ({ ...pt("ai", 650, 1500), onset: [f1a, f2a], offset: [f1b, f2b] });
  const ts = trajectories([ai(700, 1300, 400, 2000), ai(720, 1340, 420, 2100), pt("a", 700, 1300), { ...pt("e", 450, 2000), onset: [null, 1900], offset: [440, 2050] }], "hz");
  assert.deepEqual(ts, [{ label: "ai", count: 2, from: { x: 1320, y: 710 }, to: { x: 2050, y: 410 } }]);
  assert.ok(Math.abs(movement(ts[0]!) - Math.hypot(730, 300)) < 1e-9);
  // Normalized ends use the midpoints' scale, so they line up with the dots.
  const z = trajectories([ai(700, 1300, 400, 2000), pt("i", 300, 2400), pt("a", 700, 1400)], "lobanov");
  const mids = plotted([ai(700, 1300, 400, 2000), pt("i", 300, 2400), pt("a", 700, 1400)], "lobanov");
  const a = mids.find((m) => m.group === "a")!;
  assert.ok(Math.abs(z[0]!.from.y - a.y) < 1e-9, "F1 700 at the onset sits level with the a dot");
});

test("the method chosen is read back, the old on meaning Lobanov", () => {
  assert.equal(parseMethod("on"), "lobanov");
  assert.equal(parseMethod("off"), "hz");
  assert.equal(parseMethod(null), "hz");
  assert.equal(parseMethod("nearey"), "nearey");
});

test("Bark follows Traunmüller's formula", () => {
  assert.ok(Math.abs(bark(1000) - (26.81 * 1000) / 2960 + 0.53) < 1e-12);
  const [p] = plotted([pt("i", 300, 2300)], "bark");
  assert.ok(Math.abs(p!.x - bark(2300)) < 1e-12 && Math.abs(p!.y - bark(300)) < 1e-12);
});

test("Nearey: each formant's log against the speaker's mean log", () => {
  const ps = plotted([pt("i", 300, 2400), pt("a", 700, 1200)], "nearey");
  const m1 = (Math.log(300) + Math.log(700)) / 2;
  assert.ok(Math.abs(ps[0]!.y - (Math.log(300) - m1)) < 1e-12);
  assert.ok(Math.abs(ps[0]!.x + ps[1]!.x) < 1e-12, "two points sit either side of the mean");
});

test("Watt–Fabricius: over the centroid of i, a and u' at i's F1", () => {
  const pts = [pt("i", 300, 2400), pt("a", 800, 1400), pt("o", 500, 900)];
  const s = wattCentroid(pts as (Point & { f1: number; f2: number })[])!;
  assert.deepEqual([s.i, s.a], ["i", "a"]);
  assert.ok(Math.abs(s.f1 - (300 + 800 + 300) / 3) < 1e-9);
  assert.ok(Math.abs(s.f2 - (2400 + 1400 + 300) / 3) < 1e-9);
  const ps = plotted(pts, "watt-fabricius");
  assert.ok(Math.abs(ps[0]!.x - 2400 / s.f2) < 1e-12);
  // Labelled otherwise, the corners are found by where they sit.
  const found = wattCentroid([pt("I", 280, 2500), pt("A", 850, 1300), pt("U", 320, 800)] as (Point & { f1: number; f2: number })[])!;
  assert.deepEqual([found.i, found.a], ["I", "A"]);
  assert.equal(wattCentroid([pt("i", 300, 2400)] as (Point & { f1: number; f2: number })[]), null);
});

test("vowel space area: the hull of the means and its area", async () => {
  const { hull, polygonArea, vowelArea, areaLine } = await import("./logic.ts");
  const square = [{ x: 0, y: 0 }, { x: 2, y: 0 }, { x: 2, y: 2 }, { x: 0, y: 2 }, { x: 1, y: 1 }, { x: 1, y: 0 }];
  assert.equal(hull(square).length, 4);
  assert.equal(polygonArea(hull(square)), 4);
  assert.deepEqual(hull([{ x: 0, y: 0 }, { x: 1, y: 1 }, { x: 2, y: 2 }]), []);
  const g = (label: string, x: number, y: number) => ({ label, count: 3, x, y, ellipse: null });
  // A triangle /i a u/ in Hz: F2 2200–800, F1 300–750; /e/ inside it.
  // A diphthong and unlabelled segments are never corners.
  const gs = [g("i", 2200, 300), g("a", 1300, 750), g("u", 800, 320), g("e", 1800, 450), g("(no label)", 3000, 900), g("ai", 2600, 800)];
  const a = vowelArea(gs, "hz")!;
  assert.deepEqual([...a.corners].sort(), ["a", "i", "u"]);
  // Half of |(1300-2200)(320-300) - (750-300)(800-2200)| = 0.306 kHz².
  assert.ok(Math.abs(a.area - 0.306) < 1e-9);
  assert.equal(a.unit, "kHz²");
  assert.equal(vowelArea(gs, "lobanov")!.unit, "z²");
  assert.equal(vowelArea(gs.slice(0, 2), "hz"), null);
  assert.match(areaLine({ track: "A", area: a }), /^A's vowel space covers 0\.31 kHz² \(the hull of \/.\/, \/.\/, \/.\/\)\.$/);
  const b = { ...a, area: a.area * 1.5 };
  assert.match(areaLine({ track: "A", area: a }, { track: "B", area: b }), /B's vowel space covers 0\.46 kHz² .*, 1\.50 times as large\.$/);
  assert.match(areaLine({ track: "A", area: null }), /needs at least three/);
});

test("tokens far from the rest of their vowel are listed as possible outliers", async () => {
  const { outliers, outliersLine, leaveOneOut, criticalD2 } = await import("./logic.ts");
  // F(2, 3) at .99 is 30.82: against 5 others the cut is 3.2 × 30.82; with many it nears χ²(2) = 9.21.
  assert.ok(Math.abs(criticalD2(5) - 3.2 * 30.8166) < 0.01);
  assert.ok(Math.abs(criticalD2(100000) - 9.2103) < 0.01);
  assert.equal(criticalD2(2), Infinity);
  const pt = (i: number, label: string, f1: number, f2: number) => ({ segment: `s${i}`, label, startMs: 0, endMs: 100, f0: null, f1, f2, f3: null });
  // Eight /i/ around F1 300, F2 2200 and one /i/ that sounds like /a/.
  const near = [[300, 2200], [310, 2250], [290, 2150], [305, 2180], [295, 2230], [315, 2210], [285, 2190], [300, 2240]];
  const ps = [...near.map(([f1, f2], i) => pt(i, "i", f1!, f2!)), pt(99, "i", 750, 1300), pt(50, "a", 760, 1300)];
  const plotted = ps.map((p) => ({ point: p, x: p.f2, y: p.f1, group: p.label }));
  const found = outliers(plotted);
  // Left out of its own mean and spread, the stray token stands far off; no ordinary /i/ does.
  assert.deepEqual(found.map((o) => o.point.segment), ["s99"]);
  assert.ok(found[0]!.distance > 10);
  assert.equal(found[0]!.label, "i");
  // Others at (0, 1), (1, 3) and (2, 2): a point at their mean (1, 2) is 0 away; others with no spread give none.
  assert.ok(leaveOneOut({ n: 3, sx: 3, sy: 6, sxx: 5, syy: 14, sxy: 7 }, 1, 2)! < 1e-9);
  assert.equal(leaveOneOut({ n: 3, sx: 3, sy: 3, sxx: 3, syy: 3, sxy: 3 }, 2, 2), null);
  // /a/ has one token: too few to check.
  assert.match(outliersLine(found, 1), /^1 token lies further from the other tokens of its label than chance would put it \(p < \.01\)/);
  assert.match(outliersLine([], 1), /^No token lies/);
  assert.equal(outliersLine([], 0), "");
});
