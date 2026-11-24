import { test } from "node:test";
import assert from "node:assert/strict";
import { brownStage, hasPeople, isChild, mluMorphemes, mluWords, SPEAKER_COLUMNS, speakerCells, speakerColumns, speakersNote, speakersTsv, type SpeakerRow } from "./speakers.ts";

const row = (over: Partial<SpeakerRow>): SpeakerRow => ({ tier: "t", name: "CHI", texts: 1, utterances: 4, words: 10, types: 6, longest: 4, analysed: 4, morphemes: 9, mtld: null, person: "", sex: "", age: "", born: "", byText: [], ...over });

test("MLU in words and in morphemes", () => {
  assert.equal(mluWords(row({})), 2.5);
  assert.equal(mluMorphemes(row({})), 2.25);
  assert.equal(mluMorphemes(row({ analysed: 0, morphemes: 0 })), null);
  assert.equal(mluWords(row({ utterances: 0, words: 0 })), 0);
});

test("Brown's stages by their bounds", () => {
  assert.deepEqual([1.5, 2, 2.49, 2.5, 3, 3.74, 3.75, 4.5].map(brownStage), ["I", "II", "II", "III", "IV", "IV", "V", "past V"]);
});

test("a stage only for a child's track", () => {
  assert.ok(isChild("CHI") && isChild("chi2") && isChild("Target child"));
  assert.ok(!isChild("MOT") && !isChild("CHIEF"));
  assert.equal(speakerCells(row({}))[6], "II");
  assert.equal(speakerCells(row({ name: "MOT" }))[6], "");
});

test("the morpheme cell says how many utterances it covers", () => {
  assert.equal(speakerCells(row({ analysed: 2, morphemes: 6 }))[5], "3.00 (2 of 4)");
  assert.equal(speakerCells(row({ analysed: 0, morphemes: 0 }))[5], "–");
});

test("note and table", () => {
  assert.match(speakersNote([row({})]), /Brown's stages/u);
  assert.match(speakersNote([row({ analysed: 0, morphemes: 0 })]), /Gloss the sentences/u);
  assert.equal(speakersNote([]), "");
  const tsv = speakersTsv([row({})]);
  assert.equal(tsv.split("\n")[1], "CHI\t1\t4\t10\t2.50\t2.25\tII\t4\t6\t–");
  assert.equal(speakerCells(row({ mtld: 47.25 }))[9], "47.3");
});

test("a track's speaker from the People panel adds who they are", () => {
  const rows = [row({ person: "Lena", sex: "female", age: "2;06" }), row({ name: "MOT" })];
  assert.ok(hasPeople(rows) && !hasPeople([row({})]));
  assert.deepEqual(speakerColumns(rows).slice(0, 5), ["Speaker", "Spoken by", "Sex", "Age", "Texts"]);
  assert.deepEqual(speakerColumns([row({})]), SPEAKER_COLUMNS);
  const tsv = speakersTsv(rows).split("\n");
  assert.equal(tsv[1], "CHI\tLena\tfemale\t2;06\t1\t4\t10\t2.50\t2.25\tII\t4\t6\t–");
  assert.ok(tsv[2]!.startsWith("MOT\t\t\t\t1\t"));
  assert.match(speakersNote([row({})]), /double-click its name/u);
  assert.doesNotMatch(speakersNote(rows), /double-click/u);
});
