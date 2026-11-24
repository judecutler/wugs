import { test } from "node:test";
import assert from "node:assert/strict";
import { abbreviationList, abbreviationsIn, abbreviationsOf, glossHint, leipzigMeaning, sortAbbreviations, unexplainedNote } from "./abbreviations.ts";
import type { Example } from "./example.ts";

test("a gloss's grammatical parts are its abbreviations, person digits apart", () => {
  assert.deepEqual(abbreviationsIn("3SG"), ["3", "SG"]);
  assert.deepEqual(abbreviationsIn("1SG.NOM"), ["1", "SG", "NOM"]);
  assert.deepEqual(abbreviationsIn("dog.PL"), ["PL"]);
  assert.deepEqual(abbreviationsIn("1>2"), ["1", "2"]);
  assert.deepEqual(abbreviationsIn("go:PST"), ["PST"]);
  assert.deepEqual(abbreviationsIn("count"), []);
  assert.deepEqual(abbreviationsIn(""), []);
});

test("Leipzig meanings, with N- read as non-", () => {
  assert.equal(leipzigMeaning("ERG"), "ergative");
  assert.equal(leipzigMeaning("3"), "third person");
  assert.equal(leipzigMeaning("NPST"), "nonpast");
  assert.equal(leipzigMeaning("NSG"), "nonsingular");
  assert.equal(leipzigMeaning("N"), "neuter");
  assert.equal(leipzigMeaning("NEG"), "negation, negative", "a listed N word is itself");
  assert.equal(leipzigMeaning("PLU"), null);
});

test("person numbers first, then alphabetical, each once", () => {
  const ex: Example = {
    number: 1,
    translation: null,
    words: [
      { form: "a", morphs: [{ surface: "a", type: "root", gloss: "dog" }, { surface: "s", type: "suffix", gloss: "PL.ERG" }] },
      { form: "b", morphs: [{ surface: "b", type: "root", gloss: "see" }, { surface: "c", type: "suffix", gloss: "3SG.PST" }, { surface: "d", type: "suffix", gloss: "1SG" }] },
      { form: "e", morphs: [{ surface: "e", type: "root", gloss: null }] },
    ],
  };
  assert.deepEqual(abbreviationsOf([ex, ex]), ["1", "3", "ERG", "PL", "PST", "SG"]);
  assert.deepEqual(sortAbbreviations(["SG", "2", "A", "1"]), ["1", "2", "A", "SG"]);
});

test("the project's own meaning wins; an unknown one is said once", () => {
  const list = abbreviationList(["1", "AV", "PL", "PLU"], new Map([["AV", "actor voice"], ["PL", "  "]]));
  assert.deepEqual(list, [
    { abbr: "1", meaning: "first person", source: "leipzig" },
    { abbr: "AV", meaning: "actor voice", source: "project" },
    { abbr: "PL", meaning: "plural", source: "leipzig" },
    { abbr: "PLU", meaning: null, source: "none" },
  ]);
  assert.match(unexplainedNote(list) ?? "", /^PLU is not in the Leipzig list: write its meaning/u);
  assert.equal(unexplainedNote(list.slice(0, 3)), null);
});

test("a hint says what a gloss's abbreviations mean as it is typed", () => {
  assert.equal(glossHint("3SG.PST", new Map()), "3 third person · SG singular · PST past");
  assert.equal(glossHint("dog-PL", new Map()), "PL plural");
  assert.equal(glossHint("EVID.XYZ", new Map([["EVID", "evidential (reported)"]])), "EVID evidential (reported) · XYZ ?");
  assert.equal(glossHint("dog", new Map()), "");
  assert.equal(glossHint("PL.PL", new Map()), "PL plural");
});
