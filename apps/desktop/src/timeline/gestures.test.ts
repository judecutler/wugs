import assert from "node:assert/strict";
import { test } from "node:test";
import { crossedDragThreshold, dragEdge, grabOffset, hitTest, type Timed } from "./gestures.ts";
import type { Viewport } from "./viewport.ts";

// 1 px = 100 ms
const v: Viewport = { t0Ms: 0, pxPerSec: 10, durationMs: 7_200_000, widthPx: 1200 };
const seg = (id: string, startMs: number, endMs: number): Timed => ({ id, startMs, endMs });

test("a press that barely moves does not become a drag", () => {
  assert.equal(crossedDragThreshold(100, 100), false);
  assert.equal(crossedDragThreshold(100, 104), false);
  assert.equal(crossedDragThreshold(100, 96), false);
  assert.equal(crossedDragThreshold(100, 105), true);
});

test("an edge drag applies the pointer delta, not the absolute pointer time", () => {
  const offset = grabOffset(10_000, 10_300); // grabbed 300 ms right of the edge
  const moved = dragEdge("end", 5_000, 10_000, 10_800, offset, 20);
  assert.deepEqual(moved, { startMs: 5_000, endMs: 10_500 });
  const noMove = dragEdge("end", 5_000, 10_000, 10_300, offset, 20);
  assert.equal(noMove.endMs, 10_000, "a drag back to the press point leaves the edge where it was");
  assert.deepEqual(dragEdge("start", 5_000, 10_000, 5_100, grabOffset(5_000, 5_050), 20), { startMs: 5_050, endMs: 10_000 });
});

test("a dragged edge keeps the minimum length and stays at or after 0", () => {
  assert.equal(dragEdge("start", 5_000, 6_000, 9_000, 0, 20).startMs, 5_980);
  assert.equal(dragEdge("end", 5_000, 6_000, 1_000, 0, 20).endMs, 5_020);
  assert.equal(dragEdge("start", 500, 6_000, -400, 0, 20).startMs, 0);
});

test("a click inside a wide segment selects the body, and near its edge grabs the edge", () => {
  const segs = [seg("a", 10_000, 20_000)]; // x 100..200
  assert.deepEqual(hitTest(v, segs, 150), { kind: "body", id: "a" });
  assert.deepEqual(hitTest(v, segs, 103), { kind: "edge", id: "a", edge: "start" });
  assert.deepEqual(hitTest(v, segs, 205), { kind: "edge", id: "a", edge: "end" });
  assert.equal(hitTest(v, segs, 250), null);
});

test("a click on a narrow segment selects it instead of grabbing an edge", () => {
  const segs = [seg("a", 10_000, 11_000)]; // 10 px wide, under 3 edge zones
  assert.deepEqual(hitTest(v, segs, 100), { kind: "body", id: "a" });
  assert.deepEqual(hitTest(v, segs, 105), { kind: "body", id: "a" });
  assert.deepEqual(hitTest(v, segs, 110), { kind: "body", id: "a" });
  assert.deepEqual(hitTest(v, segs, 95), { kind: "edge", id: "a", edge: "start" }, "just outside still grabs the edge");
  assert.deepEqual(hitTest(v, segs, 115), { kind: "edge", id: "a", edge: "end" });
});

test("a narrow segment's body wins over a neighbour's edge zone", () => {
  const segs = [seg("wide", 0, 10_000), seg("tiny", 10_000, 11_000)]; // wide ends at x=100
  assert.deepEqual(hitTest(v, segs, 104), { kind: "body", id: "tiny" });
});
