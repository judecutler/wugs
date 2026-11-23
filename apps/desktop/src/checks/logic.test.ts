import { test } from "node:test";
import assert from "node:assert/strict";
import { addedLine, addLabel, cardNote, missingSummary, summary, unifiedLine, useLabel, type Inconsistent } from "./logic.ts";

const v = (gloss: string, count: number) => ({ gloss, count, textId: "t", textTitle: "T", tierId: "u", segmentId: "s" });
const ka: Inconsistent = { surface: "ka", kind: "suffix", key: "ka", total: 3, variants: [v("PL", 2), v("pl.", 1)], slip: true };
const mi: Inconsistent = { surface: "mi", kind: "root", key: "mi", total: 3, variants: [v("water", 2), v("drink", 1)], slip: false };

test("the summary counts morphs and slips", () => {
  assert.equal(summary([]), "Every morph is glossed one way. Nothing to check.");
  assert.equal(summary([ka, mi]), "2 morphs are glossed more than one way. 1 looks like a typing slip (the glosses differ only in capitals, spaces or dots).");
  assert.equal(summary([mi]), "1 morph is glossed more than one way.");
});

test("cards and buttons say what will happen", () => {
  assert.match(cardNote(ka), /slip/u);
  assert.match(cardNote(mi), /lexicon entry/u);
  assert.equal(useLabel(ka.variants[0]!, ka, false), "Use «PL» for all 3");
  assert.equal(useLabel(ka.variants[0]!, ka, true), "Change 1: sure?");
  assert.equal(unifiedLine(1, "PL"), "1 morph now reads «PL».");
  assert.equal(unifiedLine(0, "PL"), "Nothing needed changing any more.");
});

test("roots with no entry are counted and added in words", () => {
  assert.equal(missingSummary([]), "Every glossed root has a lexicon entry.");
  const rows = [{ surface: "mi", key: "mi", total: 3, variants: [v("water", 2), v("drink", 1)] }, { surface: "to", key: "to", total: 1, variants: [v("dog", 1)] }];
  assert.equal(missingSummary(rows), "2 roots are glossed in the texts (4 uses) but not in the lexicon yet.");
  assert.equal(addLabel(rows[0]!), "Add with 2 senses");
  assert.equal(addLabel(rows[1]!), "Add to lexicon");
  assert.equal(addedLine({ entries: 1, senses: 2, linked: 3 }), "Added 1 entry with 2 senses, and linked 3 uses in the texts to them.");
  assert.equal(addedLine({ entries: 0, senses: 0, linked: 0 }), "Nothing to add any more.");
});
