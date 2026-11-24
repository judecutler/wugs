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
  assert.match(cooccurrenceNote(c, "V"), /^72 pairs of a vowel and the next vowel in a word.*avoided/u);
  assert.match(cooccurrenceTsv(c).split("\n")[0]!, /^\t/u);
});

test("consonants across a vowel, and nothing to count", () => {
  const c = cooccurrence([{ word: "pita", count: 3 }, { word: "pita", count: 1 }], "C");
  assert.equal(c.total, 1, "a word type counts once");
  assert.equal(cooccurrenceNote(cooccurrence([{ word: "a", count: 1 }], "V"), "V"), "No word has two vowels yet.");
});
