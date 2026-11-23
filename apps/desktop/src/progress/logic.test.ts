import assert from "node:assert/strict";
import { test } from "node:test";
import { duration, percent, progressTsv, rowCells, summaryLines, totals, type Progress } from "./logic.ts";

const text = (title: string, rec: number | null, sentences: number, transcribed: number, words: number, glossed: number, translated: number) => ({
  id: title,
  title,
  recordingMs: rec,
  sentences,
  transcribed,
  transcribedMs: transcribed * 2000,
  words,
  glossedWords: glossed,
  translated,
});
const progress: Progress = {
  texts: [text("Fox", 125_000, 10, 8, 40, 30, 8), text("Market", null, 4, 0, 0, 0, 0)],
  recordedMs: 125_000,
  recordingsMissing: 1,
  entries: 12,
  senses: 15,
  sensesWithoutGloss: 2,
  people: 3,
  consentRecords: 1,
};

test("lengths and shares read the way people say them", () => {
  assert.equal(duration(null), "—");
  assert.equal(duration(40_400), "40 s");
  assert.equal(duration(750_000), "12 min 30 s");
  assert.equal(duration(7_500_000), "2 h 05 min");
  assert.equal(percent(1, 0), "—");
  assert.equal(percent(1, 300), "<1%");
  assert.equal(percent(2, 3), "67%");
});

test("the summary and table add the texts up", () => {
  const t = totals(progress);
  assert.deepEqual([t.sentences, t.transcribed, t.words, t.glossedWords], [14, 8, 40, 30]);
  const lines = summaryLines(progress);
  assert.equal(lines[0], "2 texts, 2 min 05 s recorded (1 recording is not on this computer and not counted).");
  assert.equal(lines[1], "8 of 14 sentences transcribed (57%), covering 16 s of timed speech.");
  assert.equal(lines[2], "40 words; 30 glossed (75%); 8 sentences translated (100%).");
  assert.equal(lines[3], "Lexicon: 12 entries, 15 senses, 2 still without a gloss.");
  assert.deepEqual(rowCells(progress.texts[1]!), ["Market", "—", "4", "0 (0%)", "—", "0", "0 (—)", "0 (—)"]);
  const tsv = progressTsv(progress).split("\n");
  assert.equal(tsv.length, 4);
  assert.equal(tsv[1], "Fox\t2.1\t10\t8\t0.3\t40\t30\t8");
  assert.equal(tsv[3]!.split("\t")[0], "All texts");
});
