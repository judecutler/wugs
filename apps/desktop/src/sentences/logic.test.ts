import assert from "node:assert/strict";
import { test } from "node:test";
import { frameGloss, framedLine, framePrompts, nextToDo, progressLine, shownPrompts, type PromptRow } from "./logic.ts";

const row = (id: string, answered: boolean): PromptRow => ({ id, number: Number(id), prompt: `p${id}`, answers: answered ? [{ segmentId: `s${id}`, tierId: "u", content: "x" }] : [] });
const rows = [row("1", true), row("2", false), row("3", true), row("4", false)];

test("progress and what comes next", () => {
  assert.equal(progressLine({ id: "a", name: "A", total: 4, answered: 2, textId: null }), "2 of 4 prompts answered.");
  assert.equal(progressLine({ id: "a", name: "A", total: 4, answered: 4, textId: null }), "Every one of the 4 prompts has an answer.");
  assert.deepEqual(shownPrompts(rows, true).map((r) => r.id), ["2", "4"]);
  assert.equal(shownPrompts(rows, false).length, 4);
  assert.equal(nextToDo(rows, null)?.id, "2");
  assert.equal(nextToDo(rows, "2")?.id, "4");
  assert.equal(nextToDo(rows, "4")?.id, "2", "wraps round");
  assert.equal(nextToDo([row("1", true)], "1"), null);
});

test("frames filled with each entry's gloss, entry by entry, with the headword as the hint", () => {
  const entries = [
    { id: "e1", lemma: "kuri", homograph: null, posOfSpeech: "noun", glosses: ["dog; hound"] },
    { id: "e2", lemma: "tela", homograph: 2, posOfSpeech: "noun", glosses: ["", "house"] },
    { id: "e3", lemma: "mopa", homograph: null, posOfSpeech: "verb", glosses: ["sleep"] },
    { id: "e4", lemma: "waka", homograph: null, posOfSpeech: "noun", glosses: [] },
  ];
  const f = framePrompts("I see the ___.\n\nThis is not a frame\n___", entries, "noun");
  assert.deepEqual(f.prompts, [
    { id: "e1@1", prompt: "I see the dog.", hint: "kuri" },
    { id: "e1@2", prompt: "dog", hint: "kuri" },
    { id: "e2@1", prompt: "I see the house.", hint: "tela2" },
    { id: "e2@2", prompt: "house", hint: "tela2" },
  ]);
  assert.equal(framedLine(f), "4 prompts: 2 words in 2 frames. 1 entry has no gloss to put in a frame and was left out. 1 line has no ___ and is not a frame.");
  assert.equal(framePrompts("___ sleeps", entries, null).prompts.length, 3);
  assert.match(framedLine(framePrompts("no blank", entries, null)), /^Write at least one frame/);
  assert.equal(frameGloss(["  ", "big, large"]), "big");
});
