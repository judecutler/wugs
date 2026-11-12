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
