import { test } from "node:test";
import assert from "node:assert/strict";
import { cooccurrence, cooccurrenceNote, cooccurrenceTsv, standouts } from "./cooccur.ts";

// A language with front–back vowel harmony: i and e go together, u and o go together.
const front = ["pi", "ti", "ki", "me", "le", "se"];
const back = ["pu", "tu", "ku", "mo", "lo", "so"];
const words = [...front.flatMap((a) => front.map((b) => a + b)), ...back.flatMap((a) => back.map((b) => a + b))].map((word) => ({ word, count: 1 }));

test("vowel harmony shows as pairs above and below chance", () => {
  const c = cooccurrence(words, "V");
  assert.equal(c.total, 72);
  const at = (a: string, b: string) => c.ratio[c.sounds.indexOf(a)]![c.sounds.indexOf(b)];
  assert.equal(at("i", "u"), 0, "front and back never together");
  assert.ok(at("i", "e")! > 1.8, `i…e: ${at("i", "e")}`);
  const s = standouts(c);
  assert.ok(s.some((x) => x.a === "i" && x.b === "u" && x.ratio === 0));
  assert.match(cooccurrenceNote(c, "V"), /^72 pairs of a vowel and the next vowel in a word.*never met/u);
  assert.match(cooccurrenceTsv(c).split("\n")[0]!, /^\t/u);
});

test("consonants across a vowel, and nothing to count", () => {
  const c = cooccurrence([{ word: "pita", count: 3 }, { word: "pita", count: 1 }], "C");
  assert.equal(c.total, 1, "a word type counts once");
  assert.equal(cooccurrenceNote(cooccurrence([{ word: "a", count: 1 }], "V"), "V"), "No word has two vowels yet.");
});

test("consonant before vowel: a rectangular table with gaps", () => {
  // t and k before every vowel, but ʃ only before i and e.
  const words = ["ta", "ti", "tu", "te", "ka", "ki", "ku", "ke", "ʃi", "ʃe", "ʃie", "tʃi"].flatMap((w, n) => [{ word: `${w}`, count: 1 }, { word: `${w}${"ptk"[n % 3]}a`, count: 1 }]);
  const c = cooccurrence(words, "CV");
  assert.ok(c.sounds.every((x) => !"aeiou".includes(x)), "rows are consonants");
  assert.ok(c.columns.every((x) => "aeiou".includes(x)), "columns are vowels");
  const sh = c.sounds.indexOf("ʃ");
  assert.equal(c.observed[sh]![c.columns.indexOf("u")], 0);
  assert.ok(c.observed[sh]![c.columns.indexOf("i")]! > 0);
  assert.match(cooccurrenceNote(c, "CV"), /pairs of a consonant and the vowel right after it/u);
  assert.equal(cooccurrenceTsv(c).split("\n")[0], ["", ...c.columns].join("\t"));
  assert.equal(cooccurrenceNote(cooccurrence([{ word: "a", count: 1 }], "CV"), "CV"), "No word has a consonant before a vowel yet.");
});
