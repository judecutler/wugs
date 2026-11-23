import assert from "node:assert/strict";
import { test } from "node:test";
import { nearSpellings, oneApart, pairNote, spellingSummary, countWords, didYouMean, spellIndex } from "./spelling.ts";

test("one letter apart, counted in letters with their marks", () => {
  const u = (s: string) => Array.from(new Intl.Segmenter(undefined, { granularity: "grapheme" }).segment(s), (g) => g.segment);
  assert.ok(oneApart(u("kitap"), u("kitab")));
  assert.ok(oneApart(u("kitap"), u("kitaap")));
  assert.ok(oneApart(u("kitap"), u("itap")));
  assert.ok(!oneApart(u("kitap"), u("kitap")));
  assert.ok(!oneApart(u("kitap"), u("kipat")));
  assert.ok(oneApart(u("tõka"), u("toka")), "a marked vowel is one letter");
});

test("rare words beside common ones, not two common words", () => {
  const words = [
    { word: "kesi", count: 40 },
    { word: "kési", count: 1 },
    { word: "Kesi", count: 5 },
    { word: "tekama", count: 12 },
    { word: "tekma", count: 2 },
    { word: "nikani", count: 9 },
    { word: "nikari", count: 8 },
    { word: "ŋai", count: 19 },
    { word: "nai", count: 1 },
    { word: "12345", count: 1 },
  ];
  const pairs = nearSpellings(words);
  assert.deepEqual(
    pairs.map((p) => [p.rare, p.common, p.why]),
    [
      ["kési", "kesi", "accents"],
      ["tekma", "tekama", "letter"],
    ],
  );
  assert.equal(pairs[0]!.commonCount, 45, "capitals count with their word");
  assert.equal(pairNote(pairs[1]!), "One letter added, left out or changed.");
  // ŋai/nai: different letters, and too short to judge as one letter apart.
  assert.ok(!pairs.some((p) => p.rare === "nai"));
});

test("the summary", () => {
  assert.equal(spellingSummary([], 0), "No transcribed words yet.");
  assert.match(spellingSummary([], 1200), /1,200 different words/);
  assert.match(spellingSummary([{ rare: "a", rareCount: 1, common: "b", commonCount: 9, why: "letter" }], 10), /^1 rare word is one letter/);
});

test("a typed word is checked against the project's words", () => {
  const index = spellIndex([
    { word: "kesi", count: 40 },
    { word: "Mopa", count: 12 },
    { word: "tela", count: 3 },
    { word: "kési", count: 1 },
  ]);
  assert.deepEqual(didYouMean(index, "kési"), { word: "kesi", count: 40, why: "accents" });
  assert.deepEqual(didYouMean(index, "mopaa"), { word: "mopa", count: 12, why: "letter" }, "case folded, one letter added");
  assert.deepEqual(didYouMean(index, "kezi"), { word: "kesi", count: 40, why: "letter" });
  assert.equal(didYouMean(index, "kesi"), null, "a common word is known");
  assert.equal(didYouMean(index, "telo"), null, "tela is not common enough to offer");
  assert.equal(didYouMean(index, "ab"), null);
  assert.equal(didYouMean(index, "123"), null);
  countWords(index, ["kezi", "kezi", "kezi"]);
  assert.equal(didYouMean(index, "kezi"), null, "a word used three times is the project's own");
});
