import assert from "node:assert/strict";
import { test } from "node:test";
import { changeSummary, notBroughtBack, PAD_MS, praatRange } from "./logic.ts";

test("a selected segment opens with a second either side, inside the recording", () => {
  const visible = { startMs: 0, endMs: 10_000 };
  assert.deepEqual(praatRange({ startMs: 5000, endMs: 6000 }, visible, 60_000), { startMs: 5000 - PAD_MS, endMs: 6000 + PAD_MS });
  assert.deepEqual(praatRange({ startMs: 200, endMs: 900 }, visible, 60_000), { startMs: 0, endMs: 1900 }, "not before 0");
  assert.deepEqual(praatRange({ startMs: 59_500, endMs: 59_900 }, visible, 60_000), { startMs: 58_500, endMs: 60_000 }, "not past the end");
});

test("without a timed selection the visible window opens", () => {
  const visible = { startMs: 3000, endMs: 9000 };
  assert.deepEqual(praatRange(null, visible, 60_000), visible);
  assert.deepEqual(praatRange({ startMs: null, endMs: null }, visible, 60_000), visible, "an untimed segment has no range");
  assert.deepEqual(praatRange(null, { startMs: 4000, endMs: 4000 }, 0), { startMs: 4000, endMs: 4001 }, "never empty, even with no known duration");
});

test("the summary names each kind of change once, in words", () => {
  assert.equal(changeSummary({ retimed: 0, relabelled: 0, added: 0, removed: 0 }), null);
  assert.equal(changeSummary({ retimed: 1, relabelled: 2, added: 0, removed: 0 }), "1 segment retimed, 2 labels changed");
  assert.equal(changeSummary({ retimed: 0, relabelled: 0, added: 3, removed: 1 }), "3 segments added, 1 segment removed");
});

test("what stays out of wugs is listed with counts", () => {
  assert.deepEqual(notBroughtBack([["tier added in Praat", 1], ["segment deleted in wugs since it went to Praat", 2]]), [
    "Tier added in Praat",
    "Segment deleted in wugs since it went to Praat (2)",
  ]);
});
