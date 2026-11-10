import assert from "node:assert/strict";
import { test } from "node:test";
import { analysisOf, breakdownEffect, mergeAnalysis } from "./analysis.ts";
import { normalizeLangTag } from "./lang.ts";
import type { MorphSpec } from "./suggest.ts";

const spec = (surface: string, type: string, gloss: string | null, sense: string | null = null, glossesByLang: Record<string, string> = {}): MorphSpec => ({ surface, type, gloss, glossesByLang, sense });

test("an unchanged morph keeps its glosses and sense, a new one starts bare", () => {
  const current = [spec("köpek", "root", "dog", "sen_1", { es: "perro" }), spec("ler", "suffix", "PL")];
  const merged = mergeAnalysis(
    [
      { surface: "köpek", type: "root" },
      { surface: "ler", type: "suffix" },
      { surface: "in", type: "suffix" },
    ],
    current,
  );
  assert.deepEqual(merged, [...current, spec("in", "suffix", null)]);
});

test("a changed surface or type loses its analysis; the rest keep theirs", () => {
  const current = [spec("köpek", "root", "dog"), spec("ler", "suffix", "PL")];
  const merged = mergeAnalysis([{ surface: "köpek", type: "root" }, { surface: "lar", type: "suffix" }], current);
  assert.deepEqual(merged, [current[0], spec("lar", "suffix", null)]);
  assert.deepEqual(mergeAnalysis([{ surface: "ler", type: "prefix" }], current), [spec("ler", "prefix", null)], "same surface, other type: not the same morph");
});

test("repeated surfaces are matched in order, each analysis used once", () => {
  const current = [spec("a", "root", "first"), spec("a", "root", "second")];
  const merged = mergeAnalysis([{ surface: "a", type: "root" }, { surface: "a", type: "root" }, { surface: "a", type: "root" }], current);
  assert.deepEqual(merged.map((m) => m.gloss), ["first", "second", null]);
});

test("merging never aliases the old gloss map", () => {
  const current = [spec("a", "root", "x", null, { es: "y" })];
  const merged = mergeAnalysis([{ surface: "a", type: "root" }], current);
  merged[0]!.glossesByLang.fr = "z";
  assert.deepEqual(current[0]!.glossesByLang, { es: "y" });
  assert.deepEqual(mergeAnalysis([], current), [], "an empty breakdown clears the word");
});

test("a language tag is letters, digits and hyphens, 2 to 35 characters, lowercase", () => {
  assert.equal(normalizeLangTag(" ES "), "es");
  assert.equal(normalizeLangTag("zh-Hant-TW"), "zh-hant-tw");
  assert.equal(normalizeLangTag("x"), null);
  assert.equal(normalizeLangTag("e s"), null);
  assert.equal(normalizeLangTag("e_s"), null);
  assert.equal(normalizeLangTag("a".repeat(36)), null);
  assert.equal(normalizeLangTag("a".repeat(35)), "a".repeat(35));
  assert.equal(normalizeLangTag(""), null);
});

test("analysisOf drops everything but the analysis", () => {
  const word = { morphs: [{ ...spec("köpek", "root", "dog", "sen_1", { es: "perro" }), id: "mor_1", senseLabel: "köpek \u2013 dog" }] };
  assert.deepEqual(analysisOf(word), [spec("köpek", "root", "dog", "sen_1", { es: "perro" })]);
});

test("an emptied breakdown keeps the analysis; only Clear analysis removes it", () => {
  assert.equal(breakdownEffect([], 2), "keep");
  assert.equal(breakdownEffect([], 0), "apply");
  assert.equal(breakdownEffect([{ surface: "wug", type: "root" }], 2), "apply");
});
