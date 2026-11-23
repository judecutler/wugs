import { test } from "node:test";
import assert from "node:assert/strict";
import { classOf, clustersNote, consonantClusters, gapsNote, positions, shapes, share } from "./shapes.ts";

const w = (word: string, count = 1) => ({ word, count });

test("sounds are counted where they stand in a word", () => {
  const rows = positions([w("taka", 2), w("ŋat"), w("a")], ["a", "t", "k", "ŋ"]);
  assert.deepEqual(rows, [
    { sound: "a", initial: 0, medial: 3, final: 2, alone: 1 },
    { sound: "t", initial: 2, medial: 0, final: 1, alone: 0 },
    { sound: "k", initial: 0, medial: 2, final: 0, alone: 0 },
    { sound: "ŋ", initial: 1, medial: 0, final: 0, alone: 0 },
  ]);
});

test("words are read as consonant-vowel skeletons", () => {
  assert.equal(classOf("tʰ"), "C");
  assert.equal(classOf("aː"), "V");
  assert.equal(classOf("t͡ʃ"), "C");
  assert.equal(classOf("ǃ"), "C");
  assert.equal(classOf("ł"), "?");
  assert.deepEqual(shapes([w("taka", 3), w("pata"), w("t͡ʃaːn", 2)]), [
    { shape: "CVCV", count: 4, words: 2, example: "taka" },
    { shape: "CVC", count: 2, words: 1, example: "t͡ʃaːn" },
  ]);
});

test("shares and gaps are said in words", () => {
  assert.equal(share(1, 3), "33%");
  assert.equal(share(1, 1000), "<1%");
  assert.equal(share(0, 0), "0%");
  const rows = positions([w("taŋ"), w("kaŋ"), w("paŋa"), w("bat"), w("bak")], []);
  assert.equal(gapsNote(rows), "Consonants used at least three times but never at the start of a word: ŋ.");
  assert.equal(gapsNote([]), "");
});

test("consonant clusters by position", () => {
  const words = [
    { word: "stra", count: 3 },
    { word: "apta", count: 2 },
    { word: "kast", count: 1 },
    { word: "pst", count: 9 },
    { word: "aptapta", count: 1 },
  ];
  const c = consonantClusters(words);
  assert.deepEqual(
    c.map((x) => [x.cluster, x.initial, x.medial, x.final, x.words]),
    [
      ["pt", 0, 4, 0, 2],
      ["str", 3, 0, 0, 1],
      ["st", 0, 0, 1, 1],
    ],
  );
  assert.equal(c[0]!.example, "apta");
  assert.equal(clustersNote(words, c), "Words start with up to 3 consonants before the first vowel and end with up to 2 consonants after the last. 3 clusters of two or more consonants, commonest first.");
  assert.equal(clustersNote([{ word: "ka", count: 1 }], []), "Words start with one consonant before the first vowel and end with no consonant after the last. No two consonants stand together.");
});
