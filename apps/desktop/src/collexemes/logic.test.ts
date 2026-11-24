import { test } from "node:test";
import assert from "node:assert/strict";
import { collexemeNote, collexemeTsv, narrowed, strengthCell } from "./logic.ts";

const data = { rows: [{ item: "book", together: 2, freq: 4, expected: 0.667, strength: 0.967, logLikelihood: 3.1 }, { item: "rain", together: 1, freq: 40, expected: 6.7, strength: -2.5, logLikelihood: -8 }], constructions: 3, tokens: 18, types: 2, positions: 2 };

test("the note and the table", () => {
  assert.match(collexemeNote(data, '[lemma="give"] []', 2, true), /^«\[lemma="give"\] \[\]» matches 3 times in 18 words; position 2 holds 2 different lemmas, 0 drawn to it beyond chance/u);
  assert.equal(collexemeNote({ ...data, constructions: 0 }, "x", 3, true), "The query's matches have 2 positions, so position 3 holds nothing.");
  assert.equal(collexemeNote({ ...data, tokens: 0 }, "x", 1, true), "No tagged sentences here yet.");
  assert.equal(collexemeTsv(data).split("\n")[2], "rain\t1\t40\t6.70\t-2.500\t-8.00");
  assert.equal(strengthCell(-2.5), "−2.50");
  assert.equal(strengthCell(512), "> 300");
});

test("a row narrows its slot to itself", () => {
  assert.equal(narrowed('[lemma="give"] []', 2, "book", true), '[lemma="give"] [lemma="book" %c]');
  assert.equal(narrowed('"the" [upos="ADJ"] [upos="NOUN"]', 3, "dog", false), '"the" [upos="ADJ"] [upos="NOUN" & word="dog" %c]');
  assert.equal(narrowed('[lemma="give"] [] :: match.text_genre="story"', 2, "a.b", true), '[lemma="give"] [lemma="a\\.b" %c] :: match.text_genre="story"');
  assert.equal(narrowed("[] []{0,2} []", 3, "x", false), null, "a repeat makes positions ambiguous");
  assert.equal(narrowed('"x"', 2, "y", false), null);
});
