import { test } from "node:test";
import assert from "node:assert/strict";
import { nextSteps, stepsLine } from "./next.ts";
import type { Progress } from "./logic.ts";

const text = (sentences: number, transcribed: number, translated: number, words: number, glossedWords: number) => ({ id: "t", title: "T", recordingMs: null, sentences, transcribed, transcribedMs: 0, words, glossedWords, morphs: 0, translated });
const progress = (texts: ReturnType<typeof text>[]): Progress => ({ texts, recordedMs: 0, recordingsMissing: 0, entries: 0, senses: 0, sensesWithoutGloss: 0, people: 0, consentRecords: 0 });

test("open work is listed in fieldwork order with the panel that does it", () => {
  const steps = nextSteps(progress([text(10, 8, 5, 40, 10), text(3, 3, 3, 9, 9)]), { gloss: 1, category: 4, example: 0 }, 2);
  assert.deepEqual(steps.map((s) => [s.id, s.count, s.panel]), [
    ["transcribe", 2, "transcribe"],
    ["translate", 3, "interlinear"],
    ["gloss", 30, "wordtypes"],
    ["lexicon-gloss", 1, "lexicon"],
    ["lexicon-category", 4, "lexicon"],
    ["consent", 2, "people"],
  ]);
  assert.equal(steps[3]!.text, "1 lexicon entry has a sense with no gloss.");
  assert.equal(steps[4]!.text, "4 lexicon entries have no category.");
  assert.equal(steps[4]!.list, "category");
  assert.equal(steps[0]!.text, "2 segments have no transcription yet.");
});

test("nothing open says so", () => {
  const steps = nextSteps(progress([text(2, 2, 2, 5, 5)]), {}, 0);
  assert.deepEqual(steps, []);
  assert.match(stepsLine(steps), /^Nothing waits/);
  assert.deepEqual(nextSteps(progress([]), { sound: 3 }, null).map((s) => s.text), ["3 lexicon entries are not heard in a recording yet."]);
});
