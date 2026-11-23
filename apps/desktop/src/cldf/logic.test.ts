import { test } from "node:test";
import assert from "node:assert/strict";
import { glottocodeProblem, isoProblem, publishedLines } from "./logic.ts";

test("codes are optional, and checked when typed", () => {
  assert.equal(glottocodeProblem(""), null);
  assert.equal(glottocodeProblem("stan1293"), null);
  assert.equal(glottocodeProblem(" NUCL1301 "), null, "case and spaces are forgiven, as the backend lowercases");
  assert.match(glottocodeProblem("turkish") ?? "", /four letters and four digits/u);
  assert.equal(isoProblem("tur"), null);
  assert.match(isoProblem("tr") ?? "", /three letters/u);
});

test("the report counts in words and keeps the notes", () => {
  const lines = publishedLines({ path: "/x/wugs-cldf-2026-11-23", entries: 1, senses: 2, examples: 0, restrictedLeftOut: 0, notes: ["One more thing."] });
  assert.equal(lines[0], "Saved a CLDF dataset in /x/wugs-cldf-2026-11-23.");
  assert.match(lines[1]!, /^1 entry, 2 senses and 0 glossed sentences/u);
  assert.equal(lines[2], "One more thing.");
});
