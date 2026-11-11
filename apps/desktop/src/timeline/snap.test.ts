import { test } from "node:test";
import assert from "node:assert/strict";
import { laneBounds, snapDragged, snapEdge, snapRange, SNAP_MS, type Span } from "./snap.ts";

const spans: Span[] = [
  [1000, 2000],
  [2600, 4000],
];

test("a start drawn just inside a word moves out to where speech starts", () => {
  // The "three" case: the box began 80 ms after the word did.
  assert.equal(snapEdge("start", 1080, spans), 1000);
  assert.equal(snapEdge("start", 2600 + SNAP_MS, spans), 2600);
});

test("snapping is outward only and within the tolerance", () => {
  assert.equal(snapEdge("start", 990, spans), 990, "a start before the speech start stays");
  assert.equal(snapEdge("start", 1400, spans), 1400, "too far inside: a deliberate cut");
  assert.equal(snapEdge("end", 1900, spans), 2000);
  assert.equal(snapEdge("end", 2100, spans), 2100, "an end after the speech end stays");
  assert.equal(snapEdge("end", 1600, spans), 1600);
});

test("no spans, no move", () => {
  assert.equal(snapEdge("start", 1080, []), 1080);
  assert.deepEqual(snapRange(5, 10, []), { startMs: 5, endMs: 10 });
});

test("the nearest speech edge wins", () => {
  const close: Span[] = [
    [1000, 1100],
    [1150, 1300],
  ];
  assert.equal(snapEdge("start", 1200, close), 1150);
  assert.equal(snapEdge("end", 1050, close), 1100);
});

test("a bound keeps the snap off a neighbouring segment", () => {
  assert.equal(snapEdge("start", 1080, spans, SNAP_MS, 1050), 1080);
  assert.equal(snapEdge("end", 1900, spans, SNAP_MS, 1950), 1900);
  assert.deepEqual(snapRange(1080, 3900, spans), { startMs: 1000, endMs: 4000 });
  assert.deepEqual(snapRange(1080, 3900, spans, { lo: 1020 }), { startMs: 1080, endMs: 4000 });
});

test("lane bounds are the neighbours either side, not the segment itself", () => {
  const segs = [
    { id: "a", startMs: 0, endMs: 900 },
    { id: "b", startMs: 1000, endMs: 2000 },
    { id: "c", startMs: 2500, endMs: 3000 },
    { id: "u", startMs: null, endMs: null },
  ];
  assert.deepEqual(laneBounds(segs, 1000, 2000, "b"), { lo: 900, hi: 2500 });
  assert.deepEqual(laneBounds(segs, 3100, 3200), { lo: 3000 });
  assert.deepEqual(laneBounds([], 0, 1), {});
});

test("a dragged edge snaps only when it moves outward from where it was", () => {
  // Trimming the start inward, to 100 ms after the speech start: it stays.
  assert.equal(snapDragged("start", 1100, 950, spans), 1100);
  // Dragging the start out from 1300 to 1080 (inside the word): out to 1000.
  assert.equal(snapDragged("start", 1080, 1300, spans), 1000);
  // Trimming the end in to 1900 from 2050: stays; pulling it out from 1800 to 1900: snaps.
  assert.equal(snapDragged("end", 1900, 2050, spans), 1900);
  assert.equal(snapDragged("end", 1900, 1800, spans), 2000);
  // Never past a neighbour.
  assert.equal(snapDragged("end", 1900, 1800, spans, 1950), 1900);
});
