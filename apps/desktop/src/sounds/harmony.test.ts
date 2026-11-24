import assert from "node:assert/strict";
import { test } from "node:test";
import { harmony, harmonyLines, strength } from "./harmony.ts";

const C = "ptksmnlr";
// Distinct word types: two consonants from i up front, then three vowels from the set.
const make = (vowels: string[], n: number) => Array.from({ length: n }, (_, i) => ({ word: `${C[i % 8]}${C[Math.floor(i / 8) % 8]}${vowels[i % vowels.length]}t${vowels[(i + 1) % vowels.length]}m${vowels[(i + 2) % vowels.length]}`, count: 1 }));

test("front and back vowels that never mix show backness harmony", () => {
  // Turkish-like: each word all front (i e y ø) or all back (ɯ ɑ u o).
  const words = [...make(["i", "e", "y", "ø"], 20), ...make(["ɯ", "ɑ", "u", "o"], 20)];
  const a = harmony(words);
  const back = a.find((x) => x.feature === "back")!;
  assert.equal(back.pairs, 80);
  assert.equal(back.observed, 1);
  assert.ok(Math.abs(back.expected - 0.5) < 1e-9);
  assert.equal(back.kappa, 1);
  assert.equal(strength(back.kappa), "strong");
  const lines = harmonyLines(a);
  assert.match(lines[0]!, /^Backness: 100% of 80 neighbouring vowels agree, against 50% by chance \(κ = 1\.00\): harmony in this feature is likely\./);
  // A word written twice counts once; central vowels are left out of backness.
  assert.equal(harmony([...words, ...words]).find((x) => x.feature === "back")!.pairs, 80);
  assert.equal(harmony([{ word: "kətə", count: 3 }]).find((x) => x.feature === "back")!.pairs, 0);
  // Plain a sits out of backness, æ and ɑ do not.
  assert.equal(harmony([{ word: "kata", count: 1 }]).find((x) => x.feature === "back")!.pairs, 0);
  assert.equal(harmony([{ word: "kætɑ", count: 1 }]).find((x) => x.feature === "back")!.pairs, 1);
});

test("vowels that always differ in height are a dissimilation; too few pairs say so", () => {
  const words = Array.from({ length: 40 }, (_, i) => ({ word: i % 2 === 0 ? `pi${"tak".repeat(1)}` + i : `pa${"tik"}` + i, count: 1 }));
  const height = harmony(words).find((x) => x.feature === "height")!;
  assert.equal(height.observed, 0);
  assert.ok(height.kappa < -0.2);
  assert.ok(harmonyLines(harmony(words)).some((l) => /^Height: 0% .*they tend to differ/.test(l)));
  assert.match(harmonyLines(harmony([{ word: "pata", count: 1 }]))[0]!, /^Too few words with two vowels or more/);
  assert.deepEqual(harmonyLines(harmony([{ word: "pt", count: 1 }])), []);
});
