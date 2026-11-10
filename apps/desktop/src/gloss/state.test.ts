import assert from "node:assert/strict";
import { test } from "node:test";
import { tokenState, transcriptionChanged } from "./state.ts";

test("tokens match the transcription when their forms are what the tokenizer reads", () => {
  assert.equal(tokenState("köpekler geldi.", ["köpekler", "geldi"]), "current", "punctuation is not part of a form");
  assert.equal(transcriptionChanged("köpekler geldi", ["köpekler", "geldi"]), false);
});

test("a changed word, an added word, a removed word or a retyped transcription is changed", () => {
  assert.equal(tokenState("köpekler gitti", ["köpekler", "geldi"]), "changed");
  assert.equal(tokenState("köpekler dün geldi", ["köpekler", "geldi"]), "changed");
  assert.equal(tokenState("köpekler", ["köpekler", "geldi"]), "changed");
  assert.equal(tokenState("", ["köpekler"]), "changed", "tokens outlive a cleared transcription until retokenized");
  assert.equal(transcriptionChanged("a b", ["a", "c"]), true);
});

test("no tokens yet is untokenized when there are words and empty when there are none", () => {
  assert.equal(tokenState("köpekler geldi", []), "untokenized");
  assert.equal(tokenState("", []), "empty");
  assert.equal(tokenState("  ... ", []), "empty", "punctuation alone has no words");
});

test("forms compare under NFC both ways", () => {
  assert.equal(tokenState("köpek", ["köpek"]), "current");
  assert.equal(tokenState("köpek", ["köpek"]), "current");
});
