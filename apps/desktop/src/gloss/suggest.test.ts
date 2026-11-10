import assert from "node:assert/strict";
import { test } from "node:test";
import { applyNeedsConfirm, applyOthersLabel, describe, ghostFor, ranked, suggestionsToShow, type MorphSpec, type Suggestion } from "./suggest.ts";

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
