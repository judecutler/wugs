import { test } from "node:test";
import assert from "node:assert/strict";
import { withoutTone, melodyOf, missingPairs, toneMelodies, tonesNote, tonesTsv, UNMARKED } from "./tones.ts";

test("a word's melody is its vowels' tone marks, an unmarked vowel as –", () => {
  assert.deepEqual(melodyOf("bálà")?.melody, ["H", "L"]);
  assert.deepEqual(melodyOf("kâ")?.melody, ["HL"]);
  assert.deepEqual(melodyOf("ǹdǎ")?.melody, ["LH"]);
  assert.deepEqual(melodyOf("sakā")?.melody, [UNMARKED, "M"]);
  assert.equal(melodyOf("sala")?.marked, false);
  assert.equal(melodyOf("ptk"), null);
  // Decomposed input reads the same.
  assert.deepEqual(melodyOf("bálà")?.melody, ["H", "L"]);
});

test("tone letters and tone numbers after a syllable are read as written", () => {
  assert.deepEqual(melodyOf("ma˥˩ma˧")?.melody, ["˥˩", "˧"]);
  assert.deepEqual(melodyOf("ma⁵¹ma³")?.melody, ["⁵¹", "³"]);
});

test("melodies are counted by syllables, then by word forms, with the two-syllable gaps", () => {
  const t = toneMelodies([
    { word: "bá", count: 5 },
    { word: "bà", count: 2 },
    { word: "kálá", count: 4 },
    { word: "sálà", count: 3 },
    { word: "túkà", count: 1 },
    { word: "pàlà", count: 2 },
  ]);
  assert.equal(t.types, 6);
  assert.equal(t.marked, 6);
  assert.deepEqual(t.melodies.map((m) => `${m.syllables} ${m.melody} ${m.words}`), ["1 H 1", "1 L 1", "2 H.L 2", "2 H.H 1", "2 L.L 1"]);
  assert.deepEqual(missingPairs(t), ["L.H"]);
  const note = tonesNote(t);
  assert.match(note, /6 of 6 word forms \(100%\) carry a tone mark/);
  assert.match(note, /H \(á\) \d+%/);
  assert.match(note, /never met: L\.H/);
  assert.equal(tonesTsv(t).split("\n")[1], "1\tH\t1\t5\tbá");
});

test("a spelling without tone marks says so", () => {
  const t = toneMelodies([{ word: "kanu", count: 3 }]);
  assert.equal(t.marked, 0);
  assert.match(tonesNote(t), /^No tone marks/);
});

test("without tone, á à and a are one vowel and their words merge", () => {
  assert.deepEqual(withoutTone([{ word: "bá", count: 2 }, { word: "bà", count: 3 }, { word: "ma˥˩", count: 1 }, { word: "˧", count: 1 }, { word: "ɔ̀kɔ̀", count: 1 }]), [
    { word: "ba", count: 5 },
    { word: "ma", count: 1 },
    { word: "ɔkɔ", count: 1 },
  ]);
});
