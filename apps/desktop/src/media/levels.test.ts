import assert from "node:assert/strict";
import { test } from "node:test";
import { dbLabel, levelNotes, levelsTsv, type Levels } from "./levels.ts";

const fine: Levels = { durationMs: 61_000, sampleRate: 44_100, channels: 1, peakDb: -3.2, rmsDb: -24, floorDb: -62, speechDb: -18, clips: 0, clipTimesMs: [] };

test("a well-made recording has nothing to say", () => {
  assert.deepEqual(levelNotes(fine), []);
  assert.equal(dbLabel(-3.24), "−3.2 dB");
  assert.equal(dbLabel(null), "–");
});

test("clipping, a low level, noise and a low rate are named", () => {
  const notes = levelNotes({ ...fine, clips: 3, clipTimesMs: [65_000], peakDb: -0.0, sampleRate: 11_025 });
  assert.match(notes[0]!, /^Clips 3 times, first at 1:05/);
  assert.match(notes[1]!, /11\.025 kHz/);
  assert.match(levelNotes({ ...fine, peakDb: -26 })[0]!, /^Quiet: it never comes above −26\.0 dB/);
  assert.match(levelNotes({ ...fine, floorDb: -30, speechDb: -18 })[0]!, /^Noisy: speech is only 12 dB above/);
  assert.match(levelNotes({ ...fine, peakDb: null })[0]!, /^Silent/);
});

test("the table holds a row per recording", () => {
  const tsv = levelsTsv([{ name: "Fox\tstory", levels: fine }]).split("\n");
  assert.equal(tsv[0], "Recording\tLength (s)\tSample rate\tChannels\tPeak dBFS\tLoudness dBFS\tSpeech dBFS\tRoom dBFS\tClips\tNotes");
  assert.equal(tsv[1], "Fox story\t61.0\t44100\t1\t-3.2\t-24.0\t-18.0\t-62.0\t0\t");
});
