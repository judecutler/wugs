import assert from "node:assert/strict";
import { test } from "node:test";
import { legalOnsets, syllabify, syllableNote, syllableTypes } from "./syllables.ts";

const words = [
  { word: "pata", count: 5 },
  { word: "spina", count: 2 },
  { word: "kunta", count: 3 },
  { word: "palks", count: 1 },
  { word: "aspa", count: 1 },
  { word: "ŋaːla", count: 2 },
];

test("words are cut by the onsets the language allows", () => {
  const on = legalOnsets(words);
  assert.ok(on.has("s\u0000p") && on.has("") && !on.has("n\u0000t"));
  const cut = (w: string) => syllabify(w, on)!.map((s) => s.join("")).join(".");
  assert.equal(cut("kunta"), "kun.ta", "nt cannot start a word, so n closes the first syllable");
  assert.equal(cut("aspa"), "a.spa", "sp can, so the onset takes both");
  assert.equal(cut("ŋaːla"), "ŋaː.la", "a long vowel is one sound");
  assert.equal(cut("palks"), "palks");
  assert.equal(syllabify("pst", on), null);
  assert.equal(syllabify("paX", on), null);
});

test("syllable types are counted and summed up", () => {
  const types = syllableTypes(words);
  assert.deepEqual(types.map((t) => [t.shape, t.count]), [["CV", 19], ["CCV", 3], ["CVC", 3], ["CVCCC", 1], ["V", 1]]);
  assert.equal(types.find((t) => t.shape === "CCV")!.example, "spi.na");
  assert.equal(syllableNote(types), "(C)(C)V(C)(C)(C): up to 2 consonants before the vowel and up to 3 after; 85% of syllables are open (end in a vowel). Each vowel counts as a syllable, so a diphthong written as two vowels is two.");
  assert.match(syllableNote(syllableTypes([{ word: "pa", count: 1 }])), /^CV: 1 consonant before the vowel and none after;/);
  assert.equal(syllableNote([]), "No word with a vowel to cut into syllables.");
});
