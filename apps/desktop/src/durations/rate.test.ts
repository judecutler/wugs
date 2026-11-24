import assert from "node:assert/strict";
import { test } from "node:test";
import { rateLines, speakingRate, syllableCount, type Seg } from "./rate.ts";

const seg = (id: string, start: number | null, end: number | null, content: string, tier = "t"): Seg => ({ id, tier, startMs: start, endMs: end, content });

test("a syllable for each vowel, a long vowel once", () => {
  assert.equal(syllableCount("kapa taːlu"), 4);
  assert.equal(syllableCount("mm"), 0);
});

test("articulation rate leaves the pauses out, speech rate keeps them", () => {
  const r = speakingRate([seg("a", 0, 1000, "kapa tala"), seg("b", 2000, 2500, "mitu"), seg("c", 3000, 3200, "mm"), seg("d", null, null, "kapa"), seg("e", 0, 900, "lalala", "other")], "t");
  assert.equal(r.segments, 2);
  assert.equal(r.syllables, 6);
  assert.equal(r.speechMs, 1500);
  assert.equal(r.spanMs, 2500);
  assert.deepEqual(r.perSegment, [4, 4]);
  assert.equal(r.noVowels, 1);
  assert.deepEqual(rateLines(r), [
    "Articulation rate 4.0 syllables a second: 6 syllables in 1.5 s of segments (2 segments).",
    "Per segment: median 4.0, from 4.0 to 4.0.",
    "Speech rate with the pauses between segments: 2.4 syllables a second over 2.5 s.",
    "1 segment has no vowel wugs could read and is left out.",
  ]);
  assert.match(rateLines(speakingRate([seg("c", 0, 100, "mm")], "t"))[0]!, /^No vowels found/);
  assert.equal(rateLines(speakingRate([], "t"))[0], "No labelled timed segments on this track.");
});
