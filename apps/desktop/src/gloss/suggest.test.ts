import assert from "node:assert/strict";
import { test } from "node:test";
import { applyNeedsConfirm, applyOthersLabel, describe, ghostFor, matchCase, ranked, suggestionsToShow, type MorphSpec, type Suggestion } from "./suggest.ts";

const morph = (surface: string, type: string, gloss: string | null): MorphSpec => ({ surface, type, gloss, glossesByLang: {}, sense: null });
const dogPl: Suggestion = { morphs: [morph("köpek", "root", "dog"), morph("ler", "suffix", "PL")], count: 3 };
const dogPlural: Suggestion = { morphs: [morph("köpek", "root", "dog"), morph("ler", "suffix", "PLURAL")], count: 1 };
const bare: Suggestion = { morphs: [morph("köpek", "root", null), morph("ler", "suffix", null)], count: 1 };

test("ranking puts the most used analysis first and keeps the given order on ties", () => {
  assert.deepEqual(ranked([dogPlural, dogPl, bare]).map((s) => s.count), [3, 1, 1]);
  assert.deepEqual(ranked([dogPlural, bare]), [dogPlural, bare]);
  assert.deepEqual(ranked([]), []);
});

test("a suggestion reads as a breakdown line over a gloss line", () => {
  const all = [dogPl, dogPlural];
  const line = describe(dogPl, all);
  assert.equal(line.breakdown, "köpek-ler");
  assert.equal(line.glosses, "dog-PL");
  assert.equal(line.uses, 3);
  assert.equal(line.share, 0.75);
  assert.equal(line.usesLabel, "used 3 of 4 times");
  assert.equal(describe(dogPlural, all).usesLabel, "used once");
  assert.equal(describe(dogPl, [dogPl]).usesLabel, "used 3 times");
  assert.equal(describe(bare, [bare]).glosses, "?-?", "a missing gloss shows as ?");
});

test("an unknown morph type shows as an unmarked piece", () => {
  const s: Suggestion = { morphs: [morph("gel", "stem", "come")], count: 1 };
  assert.equal(describe(s, [s]).breakdown, "gel");
});

test("only an unanalysed word gets suggestions, best first, capped", () => {
  const many = [bare, dogPlural, dogPl, { ...dogPl, count: 2 }];
  assert.deepEqual(suggestionsToShow(many, false).map((s) => s.count), [3, 2, 1]);
  assert.deepEqual(suggestionsToShow(many, false, 2).map((s) => s.count), [3, 2]);
  assert.deepEqual(suggestionsToShow(many, true), []);
  assert.deepEqual(suggestionsToShow([], false), []);
});

test("the ghost is the top suggestion, or nothing", () => {
  assert.equal(ghostFor([dogPlural, dogPl], false), dogPl);
  assert.equal(ghostFor([dogPl], true), null, "an analysed word shows no ghost");
  assert.equal(ghostFor([], false), null);
});

test("the apply-to-others link shows its count first and hides at zero", () => {
  assert.equal(applyOthersLabel("wugs", 3), "Apply to 3 other unanalysed «wugs»");
  assert.equal(applyOthersLabel("wugs", 1), "Apply to 1 other unanalysed «wugs»");
  assert.equal(applyOthersLabel("wugs", 0), null);
  assert.equal(applyOthersLabel("wugs", 40, true), "Give all 40 «wugs» this analysis? Click again");
});

test("applying to more than 20 other words takes a second click", () => {
  assert.equal(applyNeedsConfirm(20), false);
  assert.equal(applyNeedsConfirm(21), true);
});

test("an accepted suggestion takes the word's sentence-initial capital", () => {
  const fixed = matchCase("Köpekler", dogPl.morphs);
  assert.equal(fixed[0]?.surface, "Köpek");
  assert.equal(fixed[1]?.surface, "ler", "other morphs are untouched");
  assert.notEqual(fixed[0], dogPl.morphs[0], "the stored suggestion is never mutated");
  assert.equal(dogPl.morphs[0]?.surface, "köpek");
});

test("case matching changes nothing without an added capital", () => {
  assert.deepEqual(matchCase("köpekler", dogPl.morphs), dogPl.morphs, "already lower-case");
  const capStored: MorphSpec[] = [morph("Ankara", "root", "Ankara")];
  assert.deepEqual(matchCase("ankara", capStored), capStored, "a lower-case word never de-capitalizes");
  const other: MorphSpec[] = [morph("ev", "root", "house")];
  assert.deepEqual(matchCase("Köpekler", other), other, "a different first letter is left alone");
  assert.deepEqual(matchCase("", dogPl.morphs), dogPl.morphs);
  assert.deepEqual(matchCase("Köpekler", []), []);
});

test("the word's own character is copied, so a typed İ survives", () => {
  // İ (U+0130) lower-cases to i + combining dot in JS, so the fold test is
  // the function's miss case for dotted I; the plain-I case must work.
  const kir: MorphSpec[] = [morph("kır", "root", "steppe")];
  assert.equal(matchCase("Kır", kir)[0]?.surface, "Kır");
  const kis: MorphSpec[] = [morph("ışık", "root", "light")];
  assert.equal(matchCase("Işık", kis)[0]?.surface, "Işık", "dotless I is a case variant of ı");
});
