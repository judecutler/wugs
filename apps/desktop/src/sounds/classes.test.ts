import assert from "node:assert/strict";
import { test } from "node:test";
import { naturalClass } from "./classes.ts";
import { distributionNote, environments } from "./environments.ts";

test("vowels by height, backness and rounding", () => {
  assert.equal(naturalClass(["i", "e"]), "front vowels");
  assert.equal(naturalClass(["u", "o", "ɔ"]), "back vowels");
  assert.equal(naturalClass(["i", "u"]), "high vowels");
  assert.equal(naturalClass(["y", "ø"]), "rounded front vowels");
  assert.equal(naturalClass(["ɯ", "ɤ"]), "unrounded back vowels");
  assert.equal(naturalClass(["i", "a", "u"]), "vowels");
});

test("consonants by voicing, place and manner", () => {
  assert.equal(naturalClass(["p", "t", "k"]), "voiceless stops");
  assert.equal(naturalClass(["b", "d", "ɡ"]), "voiced stops");
  assert.equal(naturalClass(["m", "n", "ŋ"]), "nasals", "sonorants are voiced as a rule");
  assert.equal(naturalClass(["k", "ɡ", "ŋ"]), "velar consonants");
  assert.equal(naturalClass(["t", "s", "n"]), "alveolar consonants");
  assert.equal(naturalClass(["t", "ʃ", "ɾ"]), "coronal consonants");
  assert.equal(naturalClass(["p", "f", "s"]), "voiceless obstruents");
  assert.equal(naturalClass(["l", "r"]), "alveolar liquids");
  assert.equal(naturalClass(["m", "l", "j"]), "sonorants");
  assert.equal(naturalClass(["t͡ʃ", "d͡ʒ"]), "postalveolar affricates");
  assert.equal(naturalClass(["t͡ʃ", "ʃ"]), "voiceless postalveolar obstruents");
});

test("edges, single sounds and mixtures", () => {
  assert.equal(naturalClass(["p", "t", "#"]), "voiceless stops or a word edge");
  assert.equal(naturalClass(["i"]), null);
  assert.equal(naturalClass(["i", "#"]), null);
  assert.equal(naturalClass(["a", "t"]), null);
  assert.equal(naturalClass(["A", "B"]), null);
});

test("the environments note names what each side shares", () => {
  const words = [
    { word: "ʃi", count: 3 },
    { word: "ʃe", count: 2 },
    { word: "sa", count: 4 },
    { word: "so", count: 1 },
    { word: "su", count: 1 },
  ];
  const note = distributionNote(environments(words, "ʃ", "s"));
  assert.match(note, /«ʃ» only before i, e \(front vowels\)/u);
  assert.match(note, /«s» only before a, o, u \(vowels\)/u);
});
