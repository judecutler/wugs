import assert from "node:assert/strict";
import { test } from "node:test";
import type { Progress } from "./logic.ts";
import { pace, paceLines, worked } from "./pace.ts";
import type { Day } from "./worklog.ts";

const zero: Omit<Day, "date"> = { segments: 0, transcribed: 0, translated: 0, analysed: 0, glossed: 0, entries: 0, senses: 0, notes: 0, importedTexts: 0, importedEntries: 0, machine: 0, ops: 1, devices: 1 };
const day = (date: string, d: Partial<Day>): Day => ({ ...zero, date, ...d });
const progress: Progress = {
  texts: [
    { id: "a", title: "A", recordingMs: null, sentences: 100, transcribed: 40, transcribedMs: 0, words: 300, glossedWords: 100, morphs: 0, translated: 40 },
    { id: "b", title: "B", recordingMs: null, sentences: 20, transcribed: 20, transcribedMs: 0, words: 80, glossedWords: 80, morphs: 0, translated: 10 },
  ],
  recordedMs: 0,
  recordingsMissing: 0,
  entries: 0,
  senses: 0,
  sensesWithoutGloss: 0,
  people: 0,
  consentRecords: 0,
};

test("the pace of the last four weeks of hand work forecasts the work left", () => {
  const days = [
    day("2026-11-24", { transcribed: 12, analysed: 30 }),
    day("2026-11-20", { transcribed: 8, analysed: 10 }),
    day("2026-11-10", { transcribed: 10, translated: 4 }),
    // An import or a machine's work alone is not a working day.
    day("2026-11-09", { importedTexts: 3, machine: 40 }),
    // Too long ago to count.
    day("2026-10-01", { transcribed: 100 }),
  ];
  assert.equal(worked(days[3]!), false);
  const pc = pace(days, progress, "2026-11-24")!;
  assert.equal(pc.days, 3);
  assert.equal(pc.transcribed, 10);
  assert.equal(pc.toTranscribe, 60);
  assert.equal(pc.toTranslate, 10);
  assert.equal(pc.toGloss, 200);
  assert.deepEqual(paceLines(pc), [
    "At the pace of the last four weeks (3 working days):",
    "60 sentences still to transcribe: about 6 working days (10 a working day lately).",
    "10 sentences still to translate: about 8 working days (1.3 a working day lately).",
    "200 words still to gloss: about 15 working days (13 a working day lately).",
  ]);
  // Too few working days: nothing to forecast.
  assert.equal(pace(days.slice(0, 2), progress, "2026-11-24"), null);
  assert.deepEqual(paceLines(null), []);
});

test("work none was done on in the window says so; finished work says nothing", () => {
  const days = ["2026-11-22", "2026-11-23", "2026-11-24"].map((d) => day(d, { transcribed: 30 }));
  const lines = paceLines(pace(days, progress, "2026-11-24"));
  assert.equal(lines[1], "60 sentences still to transcribe: about 2 working days (30 a working day lately).");
  assert.equal(lines[3], "200 words still to gloss, and none were done in those four weeks.");
  const done = { ...progress, texts: [{ ...progress.texts[1]!, translated: 20 }] };
  assert.deepEqual(paceLines(pace(days, done, "2026-11-24")), []);
  assert.equal(paceLines(pace([day("2026-11-22", { transcribed: 500 }), ...days.slice(1)], progress, "2026-11-24"))[1], "60 sentences still to transcribe: less than a working day (187 a working day lately).");
});
