import assert from "node:assert/strict";
import { test } from "node:test";
import { NEW_TRACK, translationLines, type TimedSegment } from "./translation.ts";

const seg = (id: string, tier: string, s: number | null, e: number | null, content = ""): TimedSegment => ({ id, tier, startMs: s, endMs: e, content });
const tiers = [
  { id: "tx", name: "transcription", role: "transcription" },
  { id: "en", name: "English", role: "translation" },
  { id: "tr", name: "Türkçe", role: "translation" },
  { id: "nt", name: "notes", role: "note" },
];

test("the translation is the segment on a translation track that overlaps the utterance most", () => {
  const sel = seg("s", "tx", 1000, 3000);
  const segments = [sel, seg("a", "en", 0, 1500, "early"), seg("b", "en", 1500, 3500, "main"), seg("c", "en", 3500, 5000, "late"), seg("n", "nt", 1000, 3000, "a note")];
  const lines = translationLines(sel, segments, tiers);
  assert.deepEqual(lines.map((l) => [l.tier.id, l.segment?.id ?? null]), [["en", "b"], ["tr", null]], "one line per translation track; notes are not translations");
});

test("segments that only touch, or an untimed utterance, have no translation", () => {
  const sel = seg("s", "tx", 1000, 2000);
  assert.equal(translationLines(sel, [seg("a", "en", 2000, 3000)], tiers)[0]!.segment, null);
  assert.equal(translationLines(seg("u", "tx", null, null), [seg("a", "en", 0, 9000)], tiers)[0]!.segment, null);
  assert.equal(translationLines(sel, [seg("a", "en", null, null)], tiers)[0]!.segment, null);
});

test("a paired track takes the segment naming the sentence; a time-aligned one never pairs by parent", () => {
  const paired = [{ id: "pf", name: "translation", role: "translation", kind: "symbolic-association", parent: "tx" }, { id: "po", name: "other", role: "translation", kind: "symbolic-association", parent: "zz" }];
  const untimed = seg("u", "tx", null, null);
  const own = { ...seg("p", "pf", null, null, "the dog runs"), parent: "u" };
  const lines = translationLines(untimed, [untimed, { ...seg("x", "pf", null, null), parent: "v" }, own], [...tiers, ...paired]);
  assert.deepEqual(lines.map((l) => [l.tier.id, l.segment?.id ?? null]), [["pf", "p"]], "time-aligned tracks and other tiers' tracks are left out for an untimed sentence");
  const timed = seg("s", "tx", 1000, 3000);
  const both = translationLines(timed, [timed, seg("o", "en", 1000, 3000), { ...seg("q", "pf", null, null), parent: "s" }, { ...seg("r", "en", null, null), parent: "s" }], [...tiers, ...paired]);
  assert.deepEqual(both.map((l) => [l.tier.id, l.segment?.id ?? null]), [["en", "o"], ["tr", null], ["pf", "q"]]);
});

test("an untimed sentence with no paired track gets one line for a track made on first use", () => {
  const lines = translationLines(seg("u", "tx", null, null), [], tiers);
  assert.deepEqual(lines.map((l) => [l.tier.id, l.tier.parent, l.segment]), [[NEW_TRACK, "tx", null]]);
  assert.deepEqual(translationLines(seg("s", "tx", 0, 1), [], [tiers[0]!]), [], "a timed one gets the offer to add a track instead");
  assert.deepEqual(translationLines({ ...seg("t", "pf", null, null), parent: "u" }, [], tiers), [], "a translation has no translation of its own");
});
