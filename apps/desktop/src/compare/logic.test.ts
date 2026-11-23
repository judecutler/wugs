import assert from "node:assert/strict";
import { test } from "node:test";
import { compareLine, compareRows, conceptKey, edictorTsv, languagesOf, similarity, type OtherForm, type OurConcept } from "./logic.ts";

const concepts: OurConcept[] = [
  { id: "c1", number: 1, gloss: "hand", concepticonGloss: "HAND", answers: [{ lemma: "lima" }] },
  { id: "c2", number: 2, gloss: "eye", concepticonGloss: "EYE", answers: [{ lemma: "mata" }] },
  { id: "c3", number: 3, gloss: "dog", concepticonGloss: "DOG", answers: [] },
];
const others: OtherForm[] = [
  { language: "Teke", concept: "HAND", form: "nima" },
  { language: "Teke", concept: "eye", form: "maʔa" },
  { language: "Sopo", concept: "hand", form: "kubu" },
  { language: "Sopo", concept: "Dog", form: "asu" },
  { language: "Sopo", concept: "fire", form: "api" },
];

test("forms line up by concept", () => {
  assert.equal(conceptKey(" The_Hand "), "the hand");
  assert.deepEqual(languagesOf(others), ["Teke", "Sopo"]);
  const rows = compareRows(concepts, others);
  assert.deepEqual(rows[0]!.ours, ["lima"]);
  assert.deepEqual(rows[0]!.others.get("Teke"), [{ form: "nima", similarity: 0.75 }]);
  assert.equal(rows[0]!.others.get("Sopo")![0]!.similarity, 0);
  assert.equal(rows[2]!.others.get("Sopo")![0]!.similarity, null, "no answer of ours to compare");
  assert.match(compareLine(rows, languagesOf(others), others), /^2 languages beside your answers, 4 forms matched to a concept; 2 look alike .* 1 of their concepts are not in this list\./);
  assert.match(compareLine([], [], []), /^Load another language's word list/);
});

test("similarity counts sounds, not bytes", () => {
  assert.equal(similarity("tʃa", "tʃa"), 1);
  assert.equal(similarity("ka", "kaː"), 0.5, "a length mark is part of its sound");
  assert.equal(similarity("-ka", "ka"), 1, "affix markers aside");
  assert.equal(similarity("", ""), 1);
});

test("the EDICTOR file", () => {
  const tsv = edictorTsv(compareRows(concepts, others), "Kanu").split("\n");
  assert.equal(tsv[0], "ID\tDOCULECT\tCONCEPT\tFORM\tTOKENS");
  assert.equal(tsv[1], "1\tKanu\tHAND\tlima\tl i m a");
  assert.equal(tsv[2], "2\tTeke\tHAND\tnima\tn i m a");
  assert.equal(tsv.filter((l) => l.includes("\tDOG\t")).length, 1);
});
