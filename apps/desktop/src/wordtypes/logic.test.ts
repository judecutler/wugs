import { test } from "node:test";
import assert from "node:assert/strict";
import { appliedLine, meaningTitle, typedAnalysis, typesLine, withMeaning } from "./logic.ts";

test("an analysis typed for a word type", () => {
  const r = typedAnalysis("ni- kani", "1SG count");
  assert.ok(r.ok);
  if (r.ok) assert.deepEqual(r.morphs.map((m) => [m.surface, m.type, m.gloss]), [["ni", "prefix", "1SG"], ["kani", "root", "count"]]);
  const joined = typedAnalysis("ni-kani", "1SG-count");
  assert.ok(joined.ok && joined.morphs[1]!.gloss === "count");
  const clitic = typedAnalysis("kani =ma", "count=FOC");
  assert.ok(clitic.ok && clitic.morphs[1]!.type === "clitic" && clitic.morphs[1]!.gloss === "FOC");
  const bare = typedAnalysis("kani -ra", "");
  assert.ok(bare.ok && bare.morphs.every((m) => m.gloss === null));
  assert.deepEqual(typedAnalysis("ni- kani", "1SG"), { ok: false, error: "2 morphs but 1 gloss: give each morph one gloss, in order." });
  assert.equal(typedAnalysis("  ", "x").ok, false);
});

test("the words around the list", () => {
  assert.equal(typesLine(3, 0, 0), "3 word forms have uses not yet analysed, the most frequent first.");
  assert.equal(typesLine(0, 2, 7), "Every word broken into words is analysed. 2 sentences (7 words) are not broken into words yet, so their words are not listed.");
  assert.equal(appliedLine("kani", 3, 1), "Analysed 3 uses of «kani» in 1 text.");
  assert.equal(appliedLine("kani", 0, 0), "No use of «kani» was left to analyse.");
  assert.equal(appliedLine("nikani", 2, 1, 1), "Analysed 2 uses of «nikani» in 1 text. 1 morph is linked to the lexicon entry with that spelling and gloss.");
});

test("a likely meaning fills the root's gloss and keeps the rest", () => {
  assert.deepEqual(withMeaning("kuri", "", "", "dog"), { breakdown: "kuri", glosses: "dog" });
  assert.deepEqual(withMeaning("nikani", "ni- kani", "1SG x", "count"), { breakdown: "ni- kani", glosses: "1SG count" });
  assert.deepEqual(withMeaning("kanima", "kani =ma", "", "count"), { breakdown: "kani =ma", glosses: "count ?" });
  assert.deepEqual(withMeaning("kuri", "", "", "big dog"), { breakdown: "kuri", glosses: "big.dog" });
  assert.match(meaningTitle("kuri", { word: "dog", together: 3, of: 4, dice: 0.857 }), /3 of the 4 translated sentences with «kuri» \(Dice 0\.86\)/);
});
