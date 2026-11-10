import assert from "node:assert/strict";
import { test } from "node:test";
import { matchLabel, offerText, shouldOfferLexeme } from "./lexicon.ts";

const root = { surface: "köpek", type: "root", gloss: "dog", sense: null, matches: 0 };

test("a glossed root with no match and no sense offers a lexicon entry", () => {
  assert.equal(shouldOfferLexeme(root), true);
  assert.equal(offerText("köpek", " dog "), "Add «köpek» 'dog' to the lexicon");
});

test("a match, a sense link, an empty gloss, or a non-root never offers", () => {
  assert.equal(shouldOfferLexeme({ ...root, matches: 1 }), false);
  assert.equal(shouldOfferLexeme({ ...root, sense: "sen_1" }), false);
  assert.equal(shouldOfferLexeme({ ...root, gloss: "  " }), false);
  for (const type of ["suffix", "prefix", "infix", "clitic", "reduplication"]) assert.equal(shouldOfferLexeme({ ...root, type }), false, type);
  assert.equal(shouldOfferLexeme({ ...root, surface: "∅" }), false);
});

test("an all-caps grammatical gloss never offers lexicon creation, even on a root", () => {
  assert.equal(shouldOfferLexeme({ ...root, gloss: "PL" }), false);
  assert.equal(shouldOfferLexeme({ ...root, gloss: "1SG.NOM" }), false);
  assert.equal(shouldOfferLexeme({ ...root, gloss: "dog.PL" }), true, "a lexical meaning is in it");
});

test("a match reads as lemma and gloss", () => {
  assert.equal(matchLabel({ lemma: "köpek", gloss: "dog" }), "köpek – dog");
  assert.equal(matchLabel({ lemma: "köpek", gloss: "" }), "köpek");
});
