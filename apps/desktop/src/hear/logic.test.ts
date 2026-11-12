import { test } from "node:test";
import assert from "node:assert/strict";
import { planHearing } from "./logic.ts";

const text = (recording: string | null) => ({ id: "txt1", title: "Counting", recording });

test("a linked recording plays, loading it first when another is loaded", () => {
  assert.deepEqual(planHearing(text("src1"), "src1", null), { kind: "play", load: null });
  assert.deepEqual(planHearing(text("src1"), "src2", "txt1"), { kind: "play", load: "src1" });
  assert.deepEqual(planHearing(text("src1"), null, null), { kind: "play", load: "src1" });
});

test("an unlinked text plays only from the Timeline showing it", () => {
  assert.deepEqual(planHearing(text(null), "src2", "txt1"), { kind: "play", load: null });
  const other = planHearing(text(null), "src2", "txt9");
  assert.equal(other.kind, "none");
  assert.match(other.kind === "none" ? other.reason : "", /«Counting» has no recording linked/);
  assert.equal(planHearing(text(null), null, "txt1").kind, "none");
});

test("a text that is gone says so", () => {
  assert.equal(planHearing(undefined, "src1", "txt1").kind, "none");
});
