import assert from "node:assert/strict";
import { test } from "node:test";
import { askBlocker, type Exchange, historyOf, MAX_QUESTION_CHARS, shareOn, sourcesLine } from "./logic.ts";

test("a question must say something and fit", () => {
  assert.equal(askBlocker("  "), "Type a question first.");
  assert.equal(askBlocker("How do I split?"), null);
  assert.match(askBlocker("x".repeat(MAX_QUESTION_CHARS + 1)) ?? "", /up to 1000/);
});

test("only answered exchanges become history, newest six turns", () => {
  const ex = (q: string, a: string | null): Exchange => ({ question: q, answer: a, sources: [], problem: a === null ? "offline" : null });
  const h = historyOf([ex("1", "a"), ex("2", null), ex("3", "c"), ex("4", "d"), ex("5", "e")]);
  assert.equal(h.length, 6);
  assert.deepEqual(h[0], { role: "user", content: "3" });
  assert.ok(!h.some((t) => t.content === "2"), "a failed try is left out");
  assert.deepEqual(h.at(-1), { role: "assistant", content: "e" });
});

test("sources read as one line", () => {
  assert.equal(sourcesLine([]), null);
  assert.equal(sourcesLine(["Gloss › Copy an example into a paper", "Transcribe"]), "From the user guide: Gloss › Copy an example into a paper; Transcribe");
});

test("sharing is off until chosen", () => {
  assert.equal(shareOn({ getItem: () => null }), false);
  assert.equal(shareOn({ getItem: () => "on" }), true);
});

import { hopFrames, spoken } from "./logic.ts";

test("the jump starts at the top bar, peaks above both and lands in the panel", () => {
  const from = { x: 10, y: 8, width: 30, height: 30 };
  const to = { x: 400, y: 200, width: 56, height: 56 };
  const f = hopFrames(from, to, 20, 70);
  assert.deepEqual([f[0].x, f[0].y, f[0].size], [10, 8, 30]);
  const last = f.at(-1)!;
  assert.ok(Math.abs(last.x - 400) < 1e-9 && Math.abs(last.y - 200) < 1e-9 && last.size === 56);
  const highest = Math.min(...f.map((p) => p.y));
  assert.ok(highest < 8 - 60 && highest >= 8 - 71, `peak ${highest}`);
});

test("a jump down to a lower spot still rises first", () => {
  const f = hopFrames({ x: 0, y: 100, width: 40, height: 40 }, { x: 50, y: 300, width: 40, height: 40 });
  assert.ok(f[1].y < 100, "it leaves upward");
});

test("the wug says an answer a few characters at a time", () => {
  assert.equal(spoken("Press S.", 0), "");
  assert.equal(spoken("Press S.", 1000, 5), "Press");
  assert.equal(spoken("Press S.", 60_000), "Press S.");
  assert.equal(spoken("ŋ😀a", 1000, 2), "ŋ😀", "a surrogate pair is one character");
});
