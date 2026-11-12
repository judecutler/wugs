import { test } from "node:test";
import assert from "node:assert/strict";
import { planHearing, recordingOf } from "./logic.ts";

const text = (recording: string | null, id = "txt1") => ({ id, title: "Counting", recording });

test("a linked recording plays, loading it first when another is loaded", () => {
  const t = text("src1");
  assert.deepEqual(planHearing(t, [t], "src1", null), { kind: "play", load: null });
  assert.deepEqual(planHearing(t, [t], "src2", "txt1"), { kind: "play", load: "src1" });
  assert.deepEqual(planHearing(t, [t], null, null), { kind: "play", load: "src1" });
});

test("an unlinked text plays the loaded recording only on screen and only if no other text claims it", () => {
  const t = text(null);
  assert.deepEqual(planHearing(t, [t], "src2", "txt1"), { kind: "play", load: null });
  const other = planHearing(t, [t], "src2", "txt9");
  assert.equal(other.kind, "none");
  assert.match(other.kind === "none" ? other.reason : "", /«Counting» has no recording linked/);
  assert.equal(planHearing(t, [t], null, "txt1").kind, "none");
  // The sample's recording, still loaded after switching to an unlinked import.
  const sample = text("src2", "txt0");
  assert.equal(planHearing(t, [sample, t], "src2", "txt1").kind, "none");
  assert.equal(recordingOf(t, [sample, t], "src2", "txt1"), null);
});

test("a text that is gone says so", () => {
  assert.equal(planHearing(undefined, [], "src1", "txt1").kind, "none");
});
