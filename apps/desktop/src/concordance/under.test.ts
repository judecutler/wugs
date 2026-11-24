import assert from "node:assert/strict";
import { test } from "node:test";
import { linesUnder } from "./under.ts";

test("a sentence shows its translation; a translation shows its sentence", () => {
  const tiers = [
    { id: "utt", name: "utterance" },
    { id: "free", name: "translation", role: "translation", kind: "symbolic-association", parent: "utt" },
    { id: "timed", name: "English", role: "translation" },
  ];
  const segments = [
    { id: "s1", tier: "utt", startMs: null, endMs: null, content: "kuri mopa" },
    { id: "t1", tier: "free", startMs: null, endMs: null, content: "the dog sleeps", parent: "s1" },
    { id: "s2", tier: "utt", startMs: 0, endMs: 1000, content: "waka tela" },
    { id: "e2", tier: "timed", startMs: 100, endMs: 900, content: "the cat eats" },
    { id: "s3", tier: "utt", startMs: null, endMs: null, content: "senu" },
  ];
  const m = linesUnder(segments, tiers);
  assert.equal(m.get("s1"), "the dog sleeps");
  assert.equal(m.get("t1"), "kuri mopa");
  assert.equal(m.get("s2"), "the cat eats");
  assert.equal(m.get("e2"), "waka tela");
  assert.equal(m.has("s3"), false);
});
