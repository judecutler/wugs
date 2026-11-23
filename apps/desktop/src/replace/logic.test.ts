import { strict as assert } from "node:assert";
import { test } from "node:test";
import { appliedLines, previewNote, replaceLabel, undoOf, type Change } from "./logic.ts";

const change = (before: string, after: string, extra: Partial<Change> = {}): Change => ({
  textId: "t",
  textTitle: "T",
  tierId: "tr",
  segmentId: "s",
  before,
  after,
  count: 1,
  glossed: false,
  ...extra,
});

test("the undo swaps before and after and keeps the rest", () => {
  const [u] = undoOf([change("a", "b", { glossed: true })]);
  assert.equal(u?.before, "b");
  assert.equal(u?.after, "a");
  assert.equal(u?.glossed, true);
});

test("the note counts places and sentences, and says when the list is cut", () => {
  assert.match(previewNote({ changes: [], total: 0 }, ""), /exactly as typed/);
  assert.equal(previewNote({ changes: [], total: 0 }, "x"), "Nothing to change.");
  assert.equal(previewNote({ changes: [change("a", "b", { count: 2 })], total: 1 }, "x"), "2 places in 1 sentence.");
  assert.match(previewNote({ changes: [change("a", "b")], total: 3 }, "x"), /The first 1 are listed/);
  assert.equal(replaceLabel(0), "Replace");
  assert.equal(replaceLabel(4), "Replace in 4 sentences");
});

test("the toast names skipped and glossed sentences, but not glossing on the undo", () => {
  assert.deepEqual(appliedLines({ done: [change("a", "b")], skipped: 0 }), ["Replaced in 1 sentence."]);
  const lines = appliedLines({ done: [change("a", "b", { glossed: true }), change("c", "d", { glossed: true })], skipped: 1 });
  assert.equal(lines[1], "1 sentence changed since the preview and was left alone.");
  assert.match(lines[2] ?? "", /^2 glossed sentences now read/);
  assert.deepEqual(appliedLines({ done: [change("a", "b", { glossed: true })], skipped: 0 }, true), ["Put back 1 sentence."]);
});

test("headwords are counted as headwords", () => {
  const c = { textId: "", textTitle: "Lexicon «wʌn»", tierId: "", segmentId: "lex1", before: "wʌn", after: "wan", count: 1, glossed: false, field: "lemma" };
  assert.equal(previewNote({ changes: [c, { ...c, segmentId: "lex2" }], total: 2 }, "ʌ"), "2 places in 2 headwords.");
  assert.equal(replaceLabel(1, true), "Replace in 1 headword");
  const lines = appliedLines({ done: [c], skipped: 0 });
  assert.equal(lines[0], "Replaced in 1 headword.");
  assert.match(lines[1]!, /^Sentences in the texts keep the old spelling/u);
});
