import assert from "node:assert/strict";
import { test } from "node:test";
import { growthNote, growthPath, niceStep, rate, ticks, typesAt, type Growth } from "./growth.ts";

const curve = (tokens: number, f: (t: number) => number): Growth => {
  const points = Array.from({ length: 101 }, (_, i) => ({ tokens: (i * tokens) / 100, types: Math.round(f((i * tokens) / 100)) }));
  return { points, tokens, types: points[100]!.types, texts: [{ title: "A", at: 0 }] };
};

test("reading the curve", () => {
  const pts = [{ tokens: 0, types: 0 }, { tokens: 10, types: 5 }, { tokens: 20, types: 7 }];
  assert.equal(typesAt(pts, 5), 2.5);
  assert.equal(typesAt(pts, 20), 7);
  assert.equal(typesAt(pts, 99), 7);
  assert.equal(rate(pts, 10, 20), 20);
  assert.equal(rate(pts, 5, 5), 0);
});

test("the note says how fast new words still come", () => {
  assert.equal(growthNote({ points: [], tokens: 0, types: 0, texts: [] }), "No transcribed words yet.");
  assert.match(growthNote(curve(100, (t) => t / 2)), /Too few yet/);
  assert.match(growthNote(curve(10000, (t) => t / 3)), /still coming about as fast/);
  assert.match(growthNote(curve(10000, (t) => 0.5 * t ** 0.8)), /more slowly than at the start/);
  assert.match(growthNote(curve(100000, (t) => 500 * Math.log1p(t / 50))), /flattening/);
  assert.match(growthNote(curve(10000, (t) => t / 3)), /^10,000 running words, 3,333 different words \(33 per 100\)\./);
});

test("axes and the path", () => {
  assert.equal(niceStep(1000), 200);
  assert.equal(niceStep(7), 2);
  assert.equal(niceStep(0), 1);
  assert.deepEqual(ticks(1000), [0, 200, 400, 600, 800, 1000]);
  const g: Growth = { points: [{ tokens: 0, types: 0 }, { tokens: 10, types: 4 }], tokens: 10, types: 4, texts: [] };
  assert.equal(growthPath(g, { width: 110, height: 60, left: 10, right: 0, top: 0, bottom: 20 }), "M10.0,40.0 L110.0,0.0");
});
