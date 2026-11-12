import assert from "node:assert/strict";
import { test } from "node:test";
import { clock, editBlocker, NO_TRACKS, parseTracks, pxPerSecToSlider, shuttle, sliderToPxPerSec, toggleHidden, toggleLocked, toggleMuted } from "./tracks.ts";

test("track toggles flip and survive a round trip through storage", () => {
  let t = toggleLocked(toggleHidden(NO_TRACKS, "tier-a"), "tier-b");
  t = toggleMuted(t);
  assert.deepEqual(parseTracks(JSON.stringify(t)), { hidden: ["tier-a"], locked: ["tier-b"], muted: true });
  assert.deepEqual(toggleHidden(t, "tier-a").hidden, []);
  assert.deepEqual(parseTracks("not json"), NO_TRACKS);
  assert.deepEqual(parseTracks('{"hidden":[1,"x"],"muted":"yes"}'), { hidden: ["x"], locked: [], muted: false });
});

test("a locked or hidden track refuses timeline edits and says how to undo that", () => {
  const t = toggleLocked(NO_TRACKS, "tier-a");
  assert.match(editBlocker(t, "tier-a", "utterance") ?? "", /locked\. Unlock it/);
  assert.equal(editBlocker(t, "tier-b", "translation"), null);
  assert.match(editBlocker(toggleHidden(NO_TRACKS, "tier-b"), "tier-b", "translation") ?? "", /hidden/);
});

test("J, K and L shuttle as in video editors", () => {
  assert.deepEqual(shuttle("j"), { kind: "seek", byMs: -2000 });
  assert.deepEqual(shuttle("K"), { kind: "pause" });
  assert.deepEqual(shuttle("l"), { kind: "play" });
  assert.equal(shuttle("x"), null);
});

test("the clock reads like an editor's", () => {
  assert.equal(clock(0), "0:00.0");
  assert.equal(clock(62_340), "1:02.3");
  assert.equal(clock(3_725_000), "1:02:05.0");
});

test("the zoom slider covers fit to closest on a log scale and inverts", () => {
  assert.ok(Math.abs(sliderToPxPerSec(0, 10, 10_000) - 10) < 1e-9);
  assert.ok(Math.abs(sliderToPxPerSec(1000, 10, 10_000) - 10_000) < 1e-6);
  assert.ok(Math.abs(sliderToPxPerSec(500, 10, 10_000) - 316.2) < 0.1);
  assert.equal(pxPerSecToSlider(sliderToPxPerSec(300, 10, 10_000), 10, 10_000), 300);
});
