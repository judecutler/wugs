import { test } from "node:test";
import assert from "node:assert/strict";
import { morOfferLines, morReadLines } from "./mor.ts";

const r = (over: Partial<Parameters<typeof morReadLines>[0]>) => ({ withMor: 3, withGra: 3, read: 0, kept: 0, unaligned: 0, words: 0, ...over });

test("the offer names %mor and %gra", () => {
  assert.equal(morOfferLines(r({ withMor: 0, withGra: 0 })), null);
  assert.match(morOfferLines(r({}))![0]!, /^3 sentences carry CLAN's %mor and %gra\. Read them as lemmas, part-of-speech tags and dependency trees/u);
  assert.match(morOfferLines(r({ withMor: 1, withGra: 0 }))![0]!, /^One sentence carries CLAN's %mor\. Read them as lemmas, part-of-speech tags, and they/u);
  assert.match(morOfferLines(r({ withGra: 1 }))![0]!, /%mor \(1 with %gra\)/u);
});

test("what reading did", () => {
  assert.deepEqual(morReadLines(r({ read: 2, words: 6, kept: 0, unaligned: 1 })), [
    "Read 2 sentences (6 words): lemmas and tags from %mor, relations and heads from %gra as GRASP writes them. They are marked as not yet checked in the Sentence panel.",
    "One sentence did not line up word for word with its %mor (a filler or a word MOR left out) and was left alone.",
  ]);
  assert.equal(morReadLines(r({ withMor: 0 }))[0], "This text has no %mor tiers to read.");
  assert.match(morReadLines(r({ read: 0, kept: 3 })).join(" "), /No sentence was given tags\. 3 sentences already had an analysis and were left alone\./u);
});
