import { test } from "node:test";
import assert from "node:assert/strict";
import { classOf, evidenceLine, evidenceNote, glossEvidence, glossTokens } from "./glosses.ts";
import type { Affix } from "../affixes/logic.ts";

const affix = (surface: string, kind: string, gloss: string, slot: number, uses: number, categories: [string, number][]): Affix => ({
  surface,
  kind,
  gloss,
  uses,
  roots: 2,
  positions: [[slot, uses]],
  slot,
  categories,
  example: { word: "", textId: "", tierId: "", segmentId: "" },
});

test("glosses split into their grammatical tokens", () => {
  assert.deepEqual(glossTokens("3SG.SBJ-PST"), ["3SG", "SBJ", "PST"]);
  assert.deepEqual(glossTokens("go.PL"), ["PL"]);
  assert.deepEqual(glossTokens("house"), []);
});

test("root categories are read as people write them", () => {
  assert.equal(classOf("N"), "noun");
  assert.equal(classOf("vt."), "verb");
  assert.equal(classOf("pron"), "pronoun");
  assert.equal(classOf("adj"), null);
});

test("glossed affixes speak for Grambank features", () => {
  const affixes = [
    affix("lar", "suffix", "PL", 1, 120, [["n", 30]]),
    affix("ler", "suffix", "PL", 1, 80, [["n", 20]]),
    affix("di", "suffix", "PST", 1, 40, [["v", 12]]),
    affix("ni", "prefix", "1SG.SBJ", -1, 9, [["v", 4]]),
    affix("ka", "prefix", "1SG", -1, 9, [["v", 4]]),
    affix("ma", "suffix", "NEG", 2, 2, [["v", 2]]),
    affix("ya", "suffix", "DAT", 2, 10, [["adj", 3]]),
  ];
  const found = glossEvidence(affixes);
  assert.deepEqual(
    found.map((e) => e.id),
    ["GB044", "GB080", "GB083", "GB090", "GB092"],
    "plural on nouns; a verb suffix doing more than agreement; past; an S/A prefix; NEG under the minimum and DAT on adjectives say nothing; 1SG names no role",
  );
  const pl = found[0]!;
  assert.equal(pl.uses, 200);
  assert.equal(evidenceNote(pl), "From the glossed affixes: -lar PL (120 uses), -ler PL (80 uses).");
  assert.equal(evidenceNote(pl, 1), "From the glossed affixes: -lar PL (120 uses), and 1 more.");
});

test("the line above the list says what the glosses can and cannot say", () => {
  assert.match(evidenceLine(0, 0), /^No Grambank feature/);
  assert.match(evidenceLine(3, 1), /^The glossed affixes show 3 Grambank features present\. 1 of them is not yet answered/);
  assert.match(evidenceLine(1, 0), /All are answered present/);
  assert.match(evidenceLine(1, 0), /a feature not listed may still be present/);
});
