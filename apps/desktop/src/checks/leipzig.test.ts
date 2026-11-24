import assert from "node:assert/strict";
import { test } from "node:test";
import { CAPS_REASON, leipzigFix, leipzigIssues, leipzigLine, MARKS_REASON, WORDS_REASON, type GlossForm } from "./leipzig.ts";

const form = (gloss: string, count = 3, affixes = 0): GlossForm => ({ gloss, count, affixes, textId: "t", textTitle: "T", tierId: "u", segmentId: "s" });

test("several words join with full stops", () => {
  assert.deepEqual(leipzigFix(form("go out")), { form: form("go out"), to: "go.out", reasons: [WORDS_REASON] });
  assert.equal(leipzigFix(form("the man who came back late")), null, "a description is left alone");
});

test("affix glosses are written as Leipzig abbreviates them", () => {
  assert.equal(leipzigFix(form("pl", 4, 4))?.to, "PL");
  assert.equal(leipzigFix(form("plural", 4, 3))?.to, "PL");
  assert.equal(leipzigFix(form("3sg", 2, 2))?.to, "3SG");
  assert.equal(leipzigFix(form("past.neg", 2, 2))?.to, "PST.NEG");
  assert.deepEqual(leipzigFix(form("pl", 4, 4))?.reasons, [CAPS_REASON]);
  assert.equal(leipzigFix(form("past", 4, 0)), null, "a root glossed past means the past");
  assert.equal(leipzigFix(form("PL", 4, 4)), null);
  assert.equal(leipzigFix(form("a", 4, 4)), null, "one letter is left alone");
});

test("stray boundary marks come off", () => {
  const fix = leipzigFix(form("-pl", 5, 5));
  assert.equal(fix?.to, "PL");
  assert.deepEqual(fix?.reasons, [MARKS_REASON, CAPS_REASON]);
  assert.equal(leipzigFix(form("-")), null);
});

test("the issues, most used first, and the line", () => {
  const issues = leipzigIssues([form("go out", 2), form("pl", 9, 9), form("dog", 20)]);
  assert.deepEqual(issues.map((i) => i.to), ["PL", "go.out"]);
  assert.match(leipzigLine(issues, 3), /^2 glosses \(11 morphs\) could follow/u);
  assert.match(leipzigLine([], 3), /^All 3 glosses follow/u);
});
