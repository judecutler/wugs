import assert from "node:assert/strict";
import { test } from "node:test";
import { clipsLines, hitClipsLines, measuredLines, trackToMeasure } from "./logic.ts";

const tiers = [
  { id: "t1", name: "words" },
  { id: "t2", name: "utterance", utterance: true },
  { id: "t3", name: "notes" },
];
const seg = (id: string, tier: string, timed = true) => ({ id, tier, startMs: timed ? 0 : null, endMs: timed ? 100 : null });

test("the selected segment's track, else the utterance track, else the first timed one", () => {
  const segments = [seg("a", "t1"), seg("b", "t2"), seg("c", "t3", false)];
  assert.equal(trackToMeasure(tiers, segments, "a")?.id, "t1");
  assert.equal(trackToMeasure(tiers, segments, null)?.id, "t2");
  // A selected segment on a track with nothing timed falls back.
  assert.equal(trackToMeasure(tiers, segments, "c")?.id, "t2");
  assert.equal(trackToMeasure(tiers, [seg("a", "t1")], null)?.id, "t1");
  assert.equal(trackToMeasure(tiers, [seg("c", "t3", false)], null), null);
});

test("the report says what was measured and what was left out", () => {
  assert.deepEqual(measuredLines("utterance", { path: "/x/a.csv", measured: 1, untimed: 0, unvoiced: 0, analysedBy: "Praat 6.4" }), [
    "Measured 1 segment of «utterance» with Praat 6.4: saved as /x/a.csv.",
  ]);
  const lines = measuredLines("words", { path: "/x/b.csv", measured: 12, untimed: 2, unvoiced: 1, analysedBy: "Acoustic analysis (built in)" });
  assert.equal(lines.length, 3);
  assert.match(lines[1], /1 segment has no voiced frame/);
  assert.match(lines[2], /2 segments have no times and were left out/);
});

test("the clips toast names the folder and what was left out", () => {
  assert.deepEqual(clipsLines("utterance", { path: "/x/T utterance clips", clips: 1, untimed: 0 }), ["Saved 1 clip of «utterance», each named after its transcription, with clips.csv listing them: /x/T utterance clips."]);
  assert.equal(clipsLines("u", { path: "p", clips: 3, untimed: 2 })[1], "2 segments have no times and were left out.");
});

test("the toast after concordance lines were saved as clips", () => {
  assert.deepEqual(hitClipsLines({ path: "/x/kani clips", clips: 2, untimed: 1, missing: 3 }), [
    "Saved 2 sentences as sound files, each named after its transcription, with clips.csv listing the text and what was searched for: /x/kani clips.",
    "1 line has no times and was left out.",
    "3 lines' texts have no recording on this computer and were left out: pick each text's recording in the Timeline.",
  ]);
});
