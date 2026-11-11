import assert from "node:assert/strict";
import { test } from "node:test";
import { batchTargets, megabytes, progressLabel, suggestable, tint, tintTitle, correctionsLine, exportable, flaggedLine } from "./logic.ts";

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
  assert.equal(megabytes(2000), "2 KB");
  assert.equal(megabytes(5_000_000), "5.0 MB");
  assert.equal(megabytes(640_000_000), "640 MB", "the same units as the catalogue's download size");
  assert.equal(progressLabel(320_000_000, 640_000_000), "320 MB of 640 MB (50%)");
  assert.equal(progressLabel(1_000_000, 0), "1.0 MB so far");
});

test("the export line matches what the button exports", () => {
  const none = { ready: 0, recordedNo: 0, restricted: 0, unchecked: 0, unusable: 0, emptied: 0 };
  assert.equal(correctionsLine(none), "No checked suggestions to export yet.");
  assert.equal(correctionsLine({ ...none, ready: 1 }), "1 checked suggestion ready to export.");
  assert.equal(correctionsLine({ ...none, ready: 2, restricted: 3, unchecked: 1 }), "5 checked suggestions: 2 ready, 3 left out unless you include them. Also: 1 suggestion not checked yet.");
  assert.equal(correctionsLine({ ...none, ready: 1, emptied: 1 }), "1 checked suggestion ready to export. Also: 1 suggestion cleared by hand, so there is nothing to pair.");
  assert.equal(correctionsLine({ ...none, ready: 1, unusable: 2 }), "1 checked suggestion ready to export. Also: 2 clips without a recording or times, which cannot be exported.");
});

test("what the records say against is told, and included only on the person's choice", () => {
  const s = { ready: 2, recordedNo: 1, restricted: 3, unchecked: 0, unusable: 0, emptied: 0 };
  assert.equal(exportable(s, false), 2);
  assert.equal(exportable(s, true), 6);
  assert.match(flaggedLine(s) ?? "", /1 has a consent record that does not allow AI use and 3 are marked restricted/);
  assert.equal(flaggedLine({ ...s, recordedNo: 0, restricted: 0 }), null);
});
