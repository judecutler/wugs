import assert from "node:assert/strict";
import { test } from "node:test";
import { breakdownCell, breakdownKey, glossCell, glossKey, langCell, nextCell, nextWordCell } from "./keys.ts";
import type { KeyLike } from "../transcribe/logic.ts";

const key = (k: string, extra: Partial<KeyLike> = {}): KeyLike => ({ key: k, shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, isComposing: false, keyCode: 0, ...extra });
const idle = { menuOpen: false, highlighted: -1, offer: false };

test("gloss cell: Enter saves and moves on, Shift+Enter back", () => {
  assert.equal(glossKey(key("Enter"), false, idle), "commit-next");
  assert.equal(glossKey(key("Enter", { shiftKey: true }), false, idle), "commit-prev");
});

test("gloss cell: Enter picks the highlighted match, or confirms the lexicon offer", () => {
  assert.equal(glossKey(key("Enter"), false, { menuOpen: true, highlighted: 1, offer: false }), "pick");
  assert.equal(glossKey(key("Enter"), false, { menuOpen: true, highlighted: -1, offer: false }), "commit-next", "an open menu with nothing highlighted just saves");
  assert.equal(glossKey(key("Enter"), false, { menuOpen: false, highlighted: -1, offer: true }), "create-lexeme");
});

test("gloss cell: Tab and Shift+Tab move across cells", () => {
  assert.equal(glossKey(key("Tab"), false, idle), "next");
  assert.equal(glossKey(key("Tab", { shiftKey: true }), false, idle), "prev");
  assert.equal(glossKey(key("Tab"), false, { menuOpen: true, highlighted: 0, offer: true }), "next", "Tab never creates a lexeme");
});

test("gloss cell: arrows move in the menu only while it is open", () => {
  assert.equal(glossKey(key("ArrowDown"), false, idle), null);
  assert.equal(glossKey(key("ArrowDown"), false, { ...idle, menuOpen: true }), "menu-down");
  assert.equal(glossKey(key("ArrowUp"), false, { ...idle, menuOpen: true }), "menu-up");
});

test("gloss cell: Escape closes the menu or offer first, then reverts", () => {
  assert.equal(glossKey(key("Escape"), false, { ...idle, menuOpen: true }), "dismiss");
  assert.equal(glossKey(key("Escape"), false, { ...idle, offer: true }), "dismiss");
  assert.equal(glossKey(key("Escape"), false, idle), "revert");
});

test("nothing is decided while an input method composes", () => {
  const ctx = { menuOpen: true, highlighted: 0, offer: true };
  assert.equal(glossKey(key("Enter"), true, ctx), null);
  assert.equal(glossKey(key("Enter", { isComposing: true }), false, ctx), null);
  assert.equal(glossKey(key("Enter", { keyCode: 229 }), false, ctx), null, "the Enter that confirms a candidate");
  assert.equal(breakdownKey(key("Enter"), true, { empty: true, hasGhost: true, changed: false }), null);
  assert.equal(breakdownKey(key("Tab", { keyCode: 229 }), false, { empty: true, hasGhost: true, changed: false }), null);
});

test("chords and Alt are left to the browser", () => {
  assert.equal(glossKey(key("Enter", { ctrlKey: true }), false, idle), null);
  assert.equal(glossKey(key("Tab", { altKey: true }), false, idle), null);
  assert.equal(breakdownKey(key("Enter", { metaKey: true }), false, { empty: true, hasGhost: true, changed: false }), null);
});

test("breakdown field: Enter on an empty field accepts the ghost suggestion", () => {
  assert.equal(breakdownKey(key("Enter"), false, { empty: true, hasGhost: true, changed: false }), "accept-suggestion");
  assert.equal(breakdownKey(key("Enter"), false, { empty: true, hasGhost: false, changed: false }), "next");
});

test("breakdown field: typing then Enter applies it, an untouched field just moves on", () => {
  assert.equal(breakdownKey(key("Enter"), false, { empty: false, hasGhost: true, changed: true }), "apply-next", "typed text beats the ghost");
  assert.equal(breakdownKey(key("Enter"), false, { empty: false, hasGhost: false, changed: false }), "next");
  assert.equal(breakdownKey(key("Enter", { shiftKey: true }), false, { empty: false, hasGhost: false, changed: true }), "apply-prev");
  assert.equal(breakdownKey(key("Enter", { shiftKey: true }), false, { empty: true, hasGhost: true, changed: false }), "prev", "Shift+Enter never accepts");
  assert.equal(breakdownKey(key("Escape"), false, { empty: false, hasGhost: false, changed: true }), "revert");
  assert.equal(breakdownKey(key("Tab"), false, { empty: true, hasGhost: true, changed: false }), "next", "Tab skips a ghost without accepting it");
});

test("cells move in reading order across words and stop at the ends", () => {
  const order = [breakdownCell("t1"), glossCell("m1"), glossCell("m2"), langCell("m1"), breakdownCell("t2"), glossCell("m3")];
  assert.equal(nextCell(order, glossCell("m2"), 1), langCell("m1"));
  assert.equal(nextCell(order, langCell("m1"), 1), breakdownCell("t2"), "Tab crosses into the next word");
  assert.equal(nextCell(order, breakdownCell("t2"), -1), langCell("m1"));
  assert.equal(nextCell(order, glossCell("m3"), 1), null);
  assert.equal(nextCell(order, breakdownCell("t1"), -1), null);
  assert.equal(nextCell(order, "g:gone", 1), null);
});

test("settling a word moves to the next word's breakdown, skipping its gloss cells", () => {
  const order = [breakdownCell("t1"), glossCell("m1"), glossCell("m2"), breakdownCell("t2"), glossCell("m3"), breakdownCell("t3")];
  assert.equal(nextWordCell(order, breakdownCell("t1"), 1), breakdownCell("t2"));
  assert.equal(nextWordCell(order, glossCell("m3"), 1), breakdownCell("t3"));
  assert.equal(nextWordCell(order, breakdownCell("t3"), 1), null);
  assert.equal(nextWordCell(order, breakdownCell("t2"), -1), breakdownCell("t1"));
  assert.equal(nextWordCell(order, glossCell("m2"), -1), breakdownCell("t1"));
});
