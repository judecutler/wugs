import assert from "node:assert/strict";
import { test } from "node:test";
import {
  bucketPxPerSec,
  clamp,
  fitPxPerSec,
  MAX_PX_PER_SEC,
  mayRetryStrip,
  STRIP_RETRY_MS,
  pan,
  STRIP_PX,
  stripIndex,
  stripKey,
  stripMinMax,
  stripQuery,
  stripRange,
  stripSpanMs,
  stripStartMs,
  timeToX,
  visibleSpanMs,
  xToTime,
  zoomAround,
  zoomBucket,
  type Viewport,
} from "./viewport.ts";

const v: Viewport = { t0Ms: 10_000, pxPerSec: 100, durationMs: 7_200_000, widthPx: 1200 };

test("timeToX and xToTime invert each other", () => {
  for (const x of [0, 1, 617.5, 1200]) {
    assert.ok(Math.abs(timeToX(v, xToTime(v, x)) - x) < 1e-9);
  }
  assert.equal(timeToX(v, 10_000), 0);
  assert.equal(timeToX(v, 11_000), 100);
});

test("zoomAround keeps the time under the cursor fixed", () => {
  // Anchored mid-recording, where no edge clamp interferes.
  const mid: Viewport = { ...v, t0Ms: 3_600_000 };
  for (const factor of [2, 0.5, 1.4142, 0.1]) {
    const anchor = xToTime(mid, 800);
    const zoomed = zoomAround(mid, 800, factor);
    assert.ok(Math.abs(xToTime(zoomed, 800) - anchor) < 1e-6, `factor ${factor}`);
  }
  const clamped = zoomAround(v, 800, 0.001);
  assert.ok(clamped.t0Ms >= 0, "an edge clamp wins over the anchor");
});

test("zoomAround clamps instead of drifting when it hits a zoom limit", () => {
  const maxed = zoomAround(v, 600, 1e9);
  assert.equal(maxed.pxPerSec, MAX_PX_PER_SEC);
  const floored = zoomAround(v, 600, 1e-9);
  assert.equal(floored.pxPerSec, fitPxPerSec(v));
  assert.equal(floored.t0Ms, 0, "fully zoomed out starts at 0");
});

test("pan clamps to the recording", () => {
  assert.equal(pan(v, -1e9).t0Ms, 0);
  const atEnd = pan(v, 1e9);
  assert.ok(Math.abs(atEnd.t0Ms + visibleSpanMs(atEnd) - v.durationMs) < 1e-6);
  const stepped = pan(v, 250);
  assert.ok(Math.abs(stepped.t0Ms - (v.t0Ms + 2500)) < 1e-9);
});

test("clamp survives an empty recording", () => {
  const empty = clamp({ t0Ms: 50, pxPerSec: 0, durationMs: 0, widthPx: 800 });
  assert.ok(Number.isFinite(empty.pxPerSec) && empty.pxPerSec > 0);
  assert.equal(empty.t0Ms, 0);
});

test("zoom buckets quantize to powers of √2", () => {
  for (const pps of [1, 3, 10, 99.9, 100, 141.4, 5000]) {
    const snapped = bucketPxPerSec(zoomBucket(pps));
    const ratio = snapped / pps;
    assert.ok(ratio > 0.84 && ratio < 1.2, `pps ${pps} snapped to ${snapped}`);
  }
  assert.equal(zoomBucket(100), zoomBucket(105), "a small zoom step stays in its bucket");
  assert.notEqual(zoomBucket(100), zoomBucket(200));
});

test("strip indexing tiles the time axis", () => {
  const bucket = zoomBucket(100);
  const span = stripSpanMs(bucket);
  assert.equal(stripStartMs(bucket, 3), 3 * span);
  assert.equal(stripIndex(bucket, 3 * span), 3);
  assert.equal(stripIndex(bucket, 3 * span - 0.001), 2);
  assert.notEqual(stripKey(bucket, 1, 0), stripKey(bucket, 1, 1));
});

test("stripRange covers the viewport with one strip of margin", () => {
  const r = stripRange(v);
  assert.ok(stripStartMs(r.bucket, r.first) <= v.t0Ms);
  assert.ok(stripStartMs(r.bucket, r.last) >= v.t0Ms + visibleSpanMs(v));
  const atStart = stripRange({ ...v, t0Ms: 0 });
  assert.equal(atStart.first, 0, "no negative strips");
});

test("stripQuery asks for exactly one strip of pixels", () => {
  const bucket = zoomBucket(100);
  const q = stripQuery(bucket, 2, 48_000);
  assert.equal(q.pixels, STRIP_PX);
  const next = stripQuery(bucket, 3, 48_000);
  assert.equal(q.endFrame, next.startFrame, "strips tile without gaps");
  assert.ok(q.endFrame > q.startFrame);
});

test("stripMinMax aggregates the covered samples and rejects misses", () => {
  const strip = { firstFrame: 1000, bucketFrames: 10, mins: [-1, -5, -2], maxs: [1, 2, 9] };
  assert.deepEqual(stripMinMax(strip, 1000, 1030), { min: -5, max: 9 });
  assert.deepEqual(stripMinMax(strip, 1010, 1020), { min: -5, max: 2 });
  assert.deepEqual(stripMinMax(strip, 1012, 1013), { min: -5, max: 2 }, "a sub-sample range still answers");
  assert.equal(stripMinMax(strip, 0, 999), null);
  assert.equal(stripMinMax(strip, 1030, 1050), null);
});

test("a failed strip is not requested again until the backoff has passed", () => {
  assert.equal(mayRetryStrip(undefined, 5_000), true);
  assert.equal(mayRetryStrip(5_000, 5_016), false);
  assert.equal(mayRetryStrip(5_000, 5_000 + STRIP_RETRY_MS - 1), false);
  assert.equal(mayRetryStrip(5_000, 5_000 + STRIP_RETRY_MS), true);
});
