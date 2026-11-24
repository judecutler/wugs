import { test } from "node:test";
import assert from "node:assert/strict";
import { gapsLabel, nextGap, stretches, uncovered } from "./gaps.ts";

test("words close together are one stretch of speech", () => {
  assert.deepEqual(stretches([[0, 300], [400, 700], [1500, 1900]]), [[0, 700], [1500, 1900]]);
});

test("speech no segment covers is found, and short leftovers are not", () => {
  const words: [number, number][] = [[0, 900], [1000, 2000], [5000, 6000], [9000, 9200]];
  const segs = [{ startMs: 0, endMs: 1200 }, { startMs: 5800, endMs: 7000 }, { startMs: null, endMs: null }];
  assert.deepEqual(uncovered(words, segs), [[1200, 2000], [5000, 5800]], "a word of 200 ms is too short to call a gap");
  assert.deepEqual(uncovered(words, []), [[0, 2000], [5000, 6000]]);
  assert.deepEqual(uncovered(words, [{ startMs: 0, endMs: 10000 }]), []);
});

test("the next gap after the playhead, wrapping round", () => {
  const gaps: [number, number][] = [[1200, 2000], [5000, 5800]];
  assert.deepEqual(nextGap(gaps, 1500), [5000, 5800]);
  assert.deepEqual(nextGap(gaps, 6000), [1200, 2000]);
  assert.equal(nextGap([], 0), null);
  assert.equal(gapsLabel(gaps), "Next gap (2, 2 s)");
  assert.equal(gapsLabel([]), "No gaps");
});
