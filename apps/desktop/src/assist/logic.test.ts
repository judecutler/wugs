import assert from "node:assert/strict";
import { test } from "node:test";
import { batchTargets, megabytes, progressLabel, suggestable, tint, tintTitle } from "./logic.ts";

test("every machine transcription is tinted, more strongly when unsure", () => {
  assert.equal(tint({ model: "m", confidence: 0.95 }), "unsure");
  assert.equal(tint({ model: "m", confidence: 0.5 }), "unsure");
  assert.equal(tint({ model: "m", confidence: 0.49 }), "very-unsure");
  assert.match(tintTitle({ model: "wav2vec2-espeak", confidence: 0.426 }), /wav2vec2-espeak.*43% sure/);
});

const seg = (id: string, over: Partial<{ tier: string; content: string; startMs: number | null; endMs: number | null; pos: string }> = {}) => ({
  id,
  tier: "t1",
  content: "",
  startMs: 0,
  endMs: 1000,
  pos: id,
  ...over,
});

test("only empty, timed, short segments take a suggestion", () => {
  assert.equal(suggestable(seg("a")), true);
  assert.equal(suggestable(seg("a", { content: "  " })), true);
  assert.equal(suggestable(seg("a", { content: "kopek" })), false);
  assert.equal(suggestable(seg("a", { startMs: null })), false);
  assert.equal(suggestable(seg("a", { startMs: 0, endMs: 31_000 })), false);
  assert.equal(suggestable(seg("a", { startMs: 500, endMs: 500 })), false);
});

test("a batch takes one tier's empty segments in time order", () => {
  const out = batchTargets(
    [seg("c", { startMs: 3000, endMs: 4000 }), seg("a"), seg("x", { tier: "t2" }), seg("b", { startMs: 2000, endMs: 2500, content: "done" }), seg("d", { startMs: 1000, endMs: 1800 })],
    "t1",
  );
  assert.deepEqual(out.map((s) => s.id), ["a", "d", "c"]);
});

test("sizes read as people say them", () => {
  assert.equal(megabytes(2048), "2 KB");
  assert.equal(megabytes(5 * 1024 * 1024), "5.0 MB");
  assert.equal(megabytes(640 * 1024 * 1024), "640 MB");
  assert.equal(progressLabel(320 * 1024 * 1024, 640 * 1024 * 1024), "320 MB of 640 MB (50%)");
  assert.equal(progressLabel(1024 * 1024, 0), "1.0 MB so far");
});
