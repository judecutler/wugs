import assert from "node:assert/strict";
import { test } from "node:test";
import { chunksFor, frameMs, yFor, type Chunk } from "./analysis.ts";

test("a view asks for the 10 s chunks it overlaps, inside the recording", () => {
  assert.deepEqual(chunksFor(0, 8_000, 60_000), [0]);
  assert.deepEqual(chunksFor(9_500, 21_000, 60_000), [0, 1, 2]);
  assert.deepEqual(chunksFor(55_000, 70_000, 58_000), [5], "nothing past the end");
  assert.deepEqual(chunksFor(-500, 10_000, 60_000), [0], "an exact boundary does not add a chunk");
  assert.deepEqual(chunksFor(5_000, 5_000, 60_000), []);
});

test("views wider than a minute are not analysed", () => {
  assert.equal(chunksFor(0, 61_000, 7_200_000), null);
  assert.ok(chunksFor(0, 60_000, 7_200_000) !== null);
  assert.deepEqual(chunksFor(0, 600_000, 30_000), [0, 1, 2], "a short recording zoomed out still fits");
});

test("values map into the band, high values up, out of range dropped", () => {
  assert.equal(yFor("pitch", 75, 10, 100), 110);
  assert.equal(yFor("pitch", 600, 10, 100), 10);
  assert.equal(yFor("intensity", 75, 0, 100), 50);
  assert.equal(yFor("pitch", 700, 0, 100), null);
  assert.equal(yFor("formants", 0, 0, 100), 100);
});

test("frames sit in the middle of their step", () => {
  const c: Chunk = { index: 2, startMs: 20_000, stepMs: 10, pitch: [], intensity: [], f1: [], f2: [], f3: [] };
  assert.equal(frameMs(c, 0), 20_005);
  assert.equal(frameMs(c, 999), 29_995);
});
