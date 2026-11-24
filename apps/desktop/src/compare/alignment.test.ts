import { test } from "node:test";
import assert from "node:assert/strict";
import { alignAll, alignmentLine, alignmentTsv, cognateForms, kept } from "./alignment.ts";
import { judgeKey } from "./cognates.ts";
import type { Row } from "./logic.ts";

const row: Row = {
  concept: { id: "c1", number: 1, gloss: "father", concepticonGloss: "FATHER", answers: [{ lemma: "pater" }] },
  ours: ["pater"],
  others: new Map([
    ["Gothic", [{ form: "fadar", similarity: 0.6 }]],
    ["Sanskrit", [{ form: "pitar", similarity: 0.8 }]],
    ["Hittite", [{ form: "atta", similarity: 0.2 }]],
  ]),
};

test("the forms to align are the judged cognates, and the close ones when asked", () => {
  const judged = new Map([[judgeKey("Gothic", "father", "fadar"), "y" as const]]);
  const langs = ["Gothic", "Sanskrit", "Hittite"];
  assert.deepEqual(cognateForms(row, langs, judged, false).map((f) => f.form), ["fadar"]);
  assert.deepEqual(cognateForms(row, langs, judged, true).map((f) => [f.form, f.judged]), [["fadar", true], ["pitar", false]]);
});

test("each form lines up under the project's sounds", () => {
  const a = alignAll("pater", [
    { language: "Gothic", form: "fadar", judged: true },
    { language: "X", form: "spatter", judged: false },
  ]);
  assert.deepEqual(a.columns, ["p", "a", "t", "e", "r"]);
  assert.deepEqual(a.rows[0]!.cells, ["f", "a", "d", "a", "r"]);
  assert.deepEqual(a.rows[1]!.cells, ["sp", "a", "tt", "e", "r"], "an extra sound joins the next cell");
  assert.deepEqual(alignAll("ni", [{ language: "K", form: "nii", judged: true }]).rows[0]!.cells, ["n", "ii"]);
  assert.deepEqual(alignAll("pate", [{ language: "K", form: "pater", judged: true }]).rows[0]!.cells, ["p", "a", "t", "er"], "at the end, the last cell");
  assert.deepEqual(kept(a), [0, 2, 0, 1, 2]);
  assert.match(alignmentLine(a, 1), /^2 forms \(1 judged cognate\) set against «pater»: 2 of its 5 sounds/);
  assert.equal(alignmentTsv("father", a).split("\n")[2], "(this project)\tpater\tp\ta\tt\te\tr");
  assert.match(alignmentLine(alignAll("pater", []), 0), /^No form/);
});
