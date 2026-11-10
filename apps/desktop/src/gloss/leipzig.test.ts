import assert from "node:assert/strict";
import { test } from "node:test";
import { glossParts, isAllCapsGloss, isGrammatical } from "./leipzig.ts";

test("capitals and digits are grammatical, anything with lowercase is lexical", () => {
  for (const g of ["PL", "1SG", "3", "NOM", "1>2", "A3", "ÇOK"]) assert.equal(isGrammatical(g), true, g);
  for (const g of ["dog", "Ali", "pL", "", "-", ">", "1sg"]) assert.equal(isGrammatical(g), false, g);
});

test("a portmanteau stays one gloss with its dots", () => {
  assert.deepEqual(glossParts("1SG.NOM"), [
    { text: "1SG", smallCaps: true },
    { text: ".", smallCaps: false },
    { text: "NOM", smallCaps: true },
  ]);
  assert.deepEqual(glossParts("dog.PL"), [
    { text: "dog", smallCaps: false },
    { text: ".", smallCaps: false },
    { text: "PL", smallCaps: true },
  ]);
  assert.deepEqual(glossParts("dog"), [{ text: "dog", smallCaps: false }]);
  assert.deepEqual(glossParts(""), []);
});

test("joined parts read back as the original gloss", () => {
  for (const g of ["1SG.NOM", "dog.PL", "come", "PST", "a..b", ".PL"]) assert.equal(glossParts(g).map((p) => p.text).join(""), g);
});

test("only an all-grammatical gloss is an affix gloss", () => {
  assert.equal(isAllCapsGloss("PL"), true);
  assert.equal(isAllCapsGloss("1SG.NOM"), true);
  assert.equal(isAllCapsGloss(" PL "), true);
  assert.equal(isAllCapsGloss("dog.PL"), false, "a lexical meaning is in it");
  assert.equal(isAllCapsGloss("dog"), false);
  assert.equal(isAllCapsGloss(""), false);
});
