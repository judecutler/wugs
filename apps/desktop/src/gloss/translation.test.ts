import assert from "node:assert/strict";
import { test } from "node:test";
import { needsTranslationTrack, translationLines, type TimedSegment } from "./translation.ts";

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

test("a translation that names the utterance as its parent wins, timed or not", () => {
  const untimed = seg("u", "tx", null, null);
  const paired = { ...seg("p", "en", null, null, "the dog runs"), parent: "u" };
  assert.equal(translationLines(untimed, [untimed, seg("x", "en", null, null, "other"), paired], tiers)[0]!.segment?.id, "p");
  const timed = seg("s", "tx", 1000, 3000);
  const overlapping = seg("o", "en", 1000, 3000, "by time");
  assert.equal(translationLines(timed, [timed, overlapping, { ...seg("q", "en", null, null, "by parent"), parent: "s" }], tiers)[0]!.segment?.id, "q");
  assert.equal(translationLines(timed, [timed, overlapping, { ...seg("r", "en", null, null), parent: "elsewhere" }], tiers)[0]!.segment?.id, "o");
});

test("the offer to add a translation track shows only when there is none", () => {
  assert.equal(needsTranslationTrack(tiers), false);
  assert.equal(needsTranslationTrack([tiers[0]!, tiers[3]!]), true);
  assert.equal(needsTranslationTrack([]), true);
});
