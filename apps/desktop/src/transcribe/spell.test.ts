import assert from "node:assert/strict";
import { test } from "node:test";
import { matchCase, replaceWord } from "./spell.ts";

test("the offer keeps a capital the person typed", () => {
  assert.equal(matchCase("Kési", "kesi"), "Kesi");
  assert.equal(matchCase("kési", "kesi"), "kesi");
  assert.equal(matchCase("ʔa", "ʔo"), "ʔo", "a letter with no capital");
});

test("only the whole word is replaced, the first time it stands alone", () => {
  assert.equal(replaceWord("kesikési kési mopa.", "kési", "kesi"), "kesikési kesi mopa.");
  assert.equal(replaceWord("kési mopa", "kési", "kesi"), "kesi mopa", "NFD typed text is matched");
  assert.equal(replaceWord("mopa", "kési", "kesi"), "mopa");
});
