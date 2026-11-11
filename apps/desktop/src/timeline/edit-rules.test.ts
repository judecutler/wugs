import { test } from "node:test";
import assert from "node:assert/strict";
import { editRules, nextTimed, parseGranularity } from "./edit-rules.ts";

const lane = [
  { id: "c", startMs: 3000, endMs: 4000 },
  { id: "a", startMs: 0, endMs: 1000 },
  { id: "u", startMs: null, endMs: null },
  { id: "b", startMs: 1200, endMs: 2000 },
];

test("no recording: no Split (it cuts at the playhead), but Merge needs no audio", () => {
  const r = editRules(false, lane[1]!, lane);
  assert.equal(r.split, "Load a recording first.");
  assert.equal(r.merge, null);
});

test("no selected (or untimed) segment: neither", () => {
  assert.match(editRules(true, null, lane).split ?? "", /Select a segment/);
  assert.match(editRules(true, lane[2]!, lane).merge ?? "", /Select a segment/);
});

test("Merge needs a timed next segment on the same track, in time order", () => {
  assert.deepEqual(editRules(true, lane[1]!, lane), { split: null, merge: null, next: "b" });
  assert.deepEqual(editRules(true, lane[3]!, lane), { split: null, merge: null, next: "c" });
  const last = editRules(true, lane[0]!, lane);
  assert.equal(last.split, null);
  assert.match(last.merge ?? "", /no next segment/);
  assert.equal(nextTimed(lane, "missing"), null);
});

test("Auto-segment's remembered choice defaults to utterances", () => {
  assert.equal(parseGranularity(null), "utterance");
  assert.equal(parseGranularity("word"), "word");
  assert.equal(parseGranularity("junk"), "utterance");
});
