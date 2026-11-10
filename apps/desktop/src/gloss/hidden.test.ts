import assert from "node:assert/strict";
import { test } from "node:test";
import { alternativeNote, fieldLabel, hiddenNote } from "./hidden.ts";

test("fields read as words", () => {
  assert.equal(fieldLabel("gloss"), "gloss");
  assert.equal(fieldLabel("gloss:es"), "es gloss");
  assert.equal(fieldLabel("sense"), "lexicon link");
  assert.equal(fieldLabel("morphs"), "analysis");
  assert.equal(fieldLabel("x.kit.note"), "x.kit.note");
});

test("the note says what is hidden, why, and that nothing was deleted", () => {
  const morph = hiddenNote({ label: "ler", fields: ["gloss", "gloss:es"] }, "morph");
  assert.match(morph, /gloss, es gloss of the morph «ler»/);
  assert.match(morph, /leaves that morph out/);
  assert.match(morph, /nothing was deleted/);
  assert.match(hiddenNote({ label: "geldi", fields: ["morphs"] }, "word"), /analysis of the word «geldi».*segment leave it out/);
});

test("the other analysis reads as Leipzig interlinear text", () => {
  assert.equal(alternativeNote({ surfaces: ["köpek", "ler"], glosses: ["dog", "PL"] }, "word"), "Another device analysed this word as «köpek-ler 'dog-PL'».");
  assert.equal(alternativeNote({ surfaces: ["köpek", "ler"], glosses: ["dog", ""] }, "word"), "Another device analysed this word as «köpek-ler 'dog-?'».");
  assert.equal(alternativeNote({ surfaces: ["köpekler"], glosses: [""] }, "word"), "Another device analysed this word as «köpekler».");
  assert.equal(alternativeNote({ surfaces: [], glosses: [] }, "word"), "Another device cleared this word's analysis.");
  assert.equal(alternativeNote({ surfaces: ["köpek", "ler", "geldi"], glosses: [] }, "segment"), "Another device broke this segment into «köpek ler geldi».");
});
