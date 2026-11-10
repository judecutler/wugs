import assert from "node:assert/strict";
import { test } from "node:test";
import { createComposition, defaultTier, formatRange, insertAtCaret, keyAction, type KeyLike, needsCommit, neighbor, renderWindow, tierSegments } from "./logic.ts";

const seg = (id: string, startMs: number | null, endMs: number | null, tier = "t1") => ({ id, tier, startMs, endMs });

test("tierSegments filters to the tier and orders by time", () => {
  const out = tierSegments([seg("c", 3000, 4000), seg("a", 1000, 2000), seg("x", 0, 500, "t2"), seg("b", 2000, 3000)], "t1");
  assert.deepEqual(out.map((s) => s.id), ["a", "b", "c"]);
});

test("equal starts break by end, then by id", () => {
  const out = tierSegments([seg("z", 100, 200), seg("b", 100, 300), seg("a", 100, 300), seg("y", 100, 150)], "t1");
  assert.deepEqual(out.map((s) => s.id), ["y", "z", "a", "b"]);
});

test("untimed segments sort after timed ones, by id", () => {
  const out = tierSegments([seg("n2", null, null), seg("a", 5000, 6000), seg("n1", null, null)], "t1");
  assert.deepEqual(out.map((s) => s.id), ["a", "n1", "n2"]);
});

test("neighbor steps both ways and stops at the ends", () => {
  const list = tierSegments([seg("a", 0, 1), seg("b", 1, 2), seg("c", 2, 3)], "t1");
  assert.equal(neighbor(list, "a", 1)?.id, "b");
  assert.equal(neighbor(list, "c", -1)?.id, "b");
  assert.equal(neighbor(list, "c", 1), null);
  assert.equal(neighbor(list, "a", -1), null);
  assert.equal(neighbor(list, "gone", 1), null);
});

test("defaultTier prefers the transcription role, then the name, then utterance, then the first", () => {
  const t = (id: string, name: string, role: string | null = null) => ({ id, name, role });
  assert.equal(defaultTier([t("1", "Transcription"), t("2", "IPA", "transcription")])?.id, "2", "role beats name");
  assert.equal(defaultTier([t("1", "x", "translation"), t("2", "y", "transcription"), t("3", "z", "transcription")])?.id, "2");
  assert.equal(defaultTier([t("1", "gloss"), t("2", "Transcription")])?.id, "2");
  assert.equal(defaultTier([t("1", "gloss"), t("2", "utterance")])?.id, "2");
  assert.equal(defaultTier([t("1", "gloss"), t("2", "notes")])?.id, "1");
  assert.equal(defaultTier([]), null);
});

test("needsCommit is false for an untouched or restored field", () => {
  assert.equal(needsCommit("kutu", "kutu"), false);
  assert.equal(needsCommit("kutu", "kutu "), true);
  assert.equal(needsCommit("", ""), false);
  assert.equal(needsCommit("a", ""), true);
});

test("renderWindow shows everything when small, else keeps the centre inside", () => {
  assert.deepEqual(renderWindow(5, 2, 10), [0, 5]);
  assert.deepEqual(renderWindow(1000, 500, 100), [450, 550]);
  assert.deepEqual(renderWindow(1000, 0, 100), [0, 100]);
  assert.deepEqual(renderWindow(1000, 999, 100), [900, 1000]);
});

test("insertAtCaret puts a combining mark after its base", () => {
  const r = insertAtCaret("ba", 2, 2, "̃");
  assert.equal(r.value, "bã");
  assert.equal(r.caret, 3);
  assert.equal(r.value.normalize("NFC"), "bã");
});

test("insertAtCaret inserts mid-string and replaces a selection", () => {
  assert.deepEqual(insertAtCaret("kt", 1, 1, "ʃ"), { value: "kʃt", caret: 2 });
  assert.deepEqual(insertAtCaret("kxt", 2, 1, "ʔ"), { value: "kʔt", caret: 2 });
  assert.deepEqual(insertAtCaret("", 0, 0, "ː"), { value: "ː", caret: 1 });
});

test("insertAtCaret clamps offsets and never splits a surrogate pair", () => {
  assert.deepEqual(insertAtCaret("ab", 9, 9, "ː"), { value: "abː", caret: 3 });
  const astral = "a\u{1D11E}b";
  assert.equal(insertAtCaret(astral, 2, 2, "x").value, "ax\u{1D11E}b");
  assert.equal(insertAtCaret(astral, 1, 2, "x").value, "axb");
});

test("formatRange shows minutes, seconds and milliseconds, or a dash when untimed", () => {
  assert.equal(formatRange(62350, 64100), "1:02.350 – 1:04.100");
  assert.equal(formatRange(0, 5), "0:00.000 – 0:00.005");
  assert.equal(formatRange(null, 100), "—");
});

test("needsCommit treats NFC-equal text as unchanged", () => {
  assert.equal(needsCommit("café", "café"), false);
  assert.equal(needsCommit("cafe", "café"), true);
});

const key = (k: string, over: Partial<KeyLike> = {}): KeyLike => ({ key: k, shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, isComposing: false, keyCode: 0, ...over });

test("keyAction maps the loop's keys", () => {
  assert.equal(keyAction(key("Enter"), false), "commit-next");
  assert.equal(keyAction(key("Enter", { shiftKey: true }), false), "commit-prev");
  assert.equal(keyAction(key("Escape"), false), "revert");
  assert.equal(keyAction(key("Tab"), false), "replay");
  assert.equal(keyAction(key("Enter", { ctrlKey: true }), false), "replay");
  assert.equal(keyAction(key("Enter", { metaKey: true }), false), "replay");
});

test("keyAction leaves Shift+Tab, Ctrl+Space and ordinary typing alone", () => {
  assert.equal(keyAction(key("Tab", { shiftKey: true }), false), null);
  assert.equal(keyAction(key(" ", { ctrlKey: true }), false), null);
  assert.equal(keyAction(key(" ", { metaKey: true }), false), null);
  assert.equal(keyAction(key("a"), false), null);
  assert.equal(keyAction(key("Enter", { altKey: true }), false), null);
});

test("keyAction does nothing while composing, including WebKit's keyCode 229 after compositionend", () => {
  for (const k of ["Enter", "Escape", "Tab"]) {
    assert.equal(keyAction(key(k, { isComposing: true }), false), null);
    assert.equal(keyAction(key(k, { keyCode: 229 }), false), null);
    assert.equal(keyAction(key(k), true), null);
  }
  assert.equal(keyAction(key("Enter", { shiftKey: true, keyCode: 229 }), false), null);
  assert.equal(keyAction(key("Enter", { ctrlKey: true }), true), null);
});

test("the composition flag outlives compositionend by one task", () => {
  const queue: (() => void)[] = [];
  const comp = createComposition({ set: (cb) => queue.push(cb), clear: () => {} });
  assert.equal(comp.active, false);
  comp.start();
  assert.equal(comp.active, true);
  comp.end();
  assert.equal(comp.active, true);
  queue.shift()?.();
  assert.equal(comp.active, false);
});

test("a composition restarted before the task runs stays active", () => {
  const queue = new Map<number, () => void>();
  let n = 0;
  const comp = createComposition({ set: (cb) => (queue.set(++n, cb), n), clear: (id) => void queue.delete(id) });
  comp.start();
  comp.end();
  comp.start();
  queue.forEach((cb) => cb());
  assert.equal(comp.active, true);
});
