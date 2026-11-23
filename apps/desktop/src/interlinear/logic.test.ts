import assert from "node:assert/strict";
import { test } from "node:test";
import { progress, sentencesOf, type Line } from "./logic.ts";

test("sentences are the utterance track's, translations left out, in number order", () => {
  const segs = [
    { id: "b", tier: "utt", parent: null, startMs: 2000, content: "two" },
    { id: "a", tier: "utt", parent: null, startMs: 1000, content: "one" },
    { id: "t", tier: "tr", parent: "a", startMs: null, content: "uno" },
    { id: "x", tier: "other", parent: null, startMs: 500, content: "x" },
  ];
  const numbers = new Map([["a", 1], ["b", 2]]);
  assert.deepEqual(sentencesOf(segs, [{ id: "utt", utterance: true }, { id: "tr", utterance: false }], numbers).map((s) => s.id), ["a", "b"]);
  assert.deepEqual(sentencesOf(segs.slice(2), [], new Map()).map((s) => s.id), ["x"], "no utterance track: the first track with sentences");
});

test("progress counts sentences and words", () => {
  const w = (n: number) => ({ form: "w", morphs: Array.from({ length: n }, () => ({ surface: "w", type: "root", gloss: "g" })) });
  const lines: Line[] = [
    { id: "a", number: 1, content: "", words: [w(1), w(0)], translation: null, startMs: null },
    { id: "b", number: 2, content: "", words: [w(0)], translation: null, startMs: null },
  ];
  assert.equal(progress(lines), "1 of 2 sentences glossed; 1 of 3 words (33%).");
  assert.equal(progress([]), "This text has no sentences yet.");
});
