import assert from "node:assert/strict";
import { test } from "node:test";
import { keyLabel, moveIndex, opensPalette, opensShortcuts, rank, score, SHORTCUTS, type Command } from "./logic.ts";

const cmd = (id: string, title: string, group: string, words?: string[]): Command => ({ id, title, group, words, run: () => {} });
const all = [
  cmd("split", "Split at the playhead", "Timeline", ["cut"]),
  cmd("night", "Colours: Night", "Look", ["theme", "dark"]),
  cmd("lex", "Show Lexicon", "Panels", ["open"]),
  cmd("settings", "Open Settings", "wugs", ["preferences"]),
];

test("every query word must be found; word starts count more", () => {
  assert.equal(score("spl", "Split at the playhead"), 3);
  assert.equal(score("split", "Split at the playhead"), 4);
  assert.equal(score("split zzz", "Split at the playhead"), 0);
  assert.ok(score("head", "Split at the playhead") > 0);
  assert.equal(score("", "anything"), 1);
});

test("accents and case are ignored", () => {
  assert.ok(score("lexico", "Show Léxicon") > 0);
  assert.ok(score("NIGHT", "Colours: Night") > 0);
});

test("ranking finds by title, group and extra words, title first", () => {
  assert.deepEqual(rank("cut", all).map((c) => c.id), ["split"]);
  assert.deepEqual(rank("dark", all).map((c) => c.id), ["night"]);
  assert.deepEqual(rank("lex", all).map((c) => c.id), ["lex"]);
  assert.equal(rank("", all).length, all.length);
  // With nothing typed, the work at hand comes before panels and looks.
  assert.equal(rank("", all)[0]?.id, "split");
  assert.deepEqual(rank("open", all).map((c) => c.id), ["settings", "lex"]);
});

test("arrow keys wrap", () => {
  assert.equal(moveIndex(0, -1, 4), 3);
  assert.equal(moveIndex(3, 1, 4), 0);
  assert.equal(moveIndex(0, 1, 0), 0);
});

test("Ctrl+K or Cmd+K opens the palette; ? opens shortcuts only outside text fields", () => {
  const k = { key: "k", ctrlKey: true, metaKey: false, altKey: false, shiftKey: false };
  assert.ok(opensPalette(k));
  assert.ok(!opensPalette({ ...k, ctrlKey: false, metaKey: true }));
  assert.ok(opensPalette({ ...k, ctrlKey: false, metaKey: true }, true));
  // On a Mac, Ctrl+K stays the text field's delete-to-end-of-line.
  assert.ok(!opensPalette(k, true));
  assert.ok(!opensPalette({ ...k, ctrlKey: false }));
  assert.ok(!opensPalette({ ...k, shiftKey: true }));
  const q = { key: "?", ctrlKey: false, metaKey: false, altKey: false };
  assert.ok(opensShortcuts(q, false));
  assert.ok(!opensShortcuts(q, true));
});

test("keys read the Mac way on a Mac", () => {
  assert.equal(keyLabel("Ctrl+Shift+Z", true), "⌘⇧Z");
  assert.equal(keyLabel("Ctrl+K", false), "Ctrl+K");
});

test("the shortcuts sheet lists the timeline's keys", () => {
  const timeline = SHORTCUTS.find((g) => g.where.startsWith("Timeline"));
  assert.ok(timeline?.keys.some(([k]) => k === "S"));
});
