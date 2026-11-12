import assert from "node:assert/strict";
import { test } from "node:test";
import { axesOf, cellFeatures, cellKey, cellLabel, coverageLine, layout, otherFormLine, templateById, TEMPLATES } from "./paradigms.ts";

const pn = templateById("person-number");
const form = (id: string, surface: string, features: Record<string, string>) => ({ id, form: surface, features });

test("template ids are unique and an unknown id falls back to the first", () => {
  assert.equal(new Set(TEMPLATES.map((t) => t.id)).size, TEMPLATES.length);
  assert.equal(templateById("nope").id, TEMPLATES[0]!.id);
  assert.equal(templateById(null).id, TEMPLATES[0]!.id);
});

test("the commands get rows first, then columns, as values", () => {
  assert.deepEqual(axesOf(pn), [
    { feature: "person", values: ["1", "2", "3"] },
    { feature: "number", values: ["sg", "pl"] },
  ]);
  assert.deepEqual(axesOf(templateById("number")), [{ feature: "number", values: ["sg", "pl"] }]);
});

test("cells are placed by their features, not by the order they came in", () => {
  const cells = [
    { features: { person: "3", number: "pl" }, forms: [form("f2", "gidiyorlar", { person: "3", number: "pl" })] },
    { features: { person: "1", number: "sg" }, forms: [form("f1", "gidiyorum", { person: "1", number: "sg" })] },
    { features: { person: "1", number: "pl" }, forms: [] },
  ];
  const grid = layout(pn, cells);
  assert.equal(grid.length, 3);
  assert.equal(grid[0]![0]!.forms[0]!.form, "gidiyorum");
  assert.equal(grid[2]![1]!.forms[0]!.form, "gidiyorlar");
  assert.equal(grid[1]![0], null, "a cell the answer lacks is empty");
  assert.equal(coverageLine(pn, cells), "2 of 6 cells filled");
});

test("an answer for another template fills nothing", () => {
  const tensePerson = [{ features: { tense: "prs", person: "1" }, forms: [form("f", "x", { tense: "prs", person: "1" })] }];
  assert.equal(coverageLine(pn, tensePerson), "0 of 6 cells filled");
});

test("a one-axis template is one column", () => {
  const t = templateById("number");
  const grid = layout(t, [{ features: { number: "pl" }, forms: [form("f", "evler", { number: "pl" })] }]);
  assert.deepEqual(grid.map((r) => r.length), [1, 1]);
  assert.equal(grid[1]![0]!.forms[0]!.form, "evler");
  assert.deepEqual(cellFeatures(t, "sg", null), { number: "sg" });
});

test("cells and stray forms read in words", () => {
  assert.equal(cellLabel(pn, { person: "1", number: "sg" }), "1st person singular");
  assert.equal(cellLabel(templateById("number"), { number: "pl" }), "plural");
  assert.equal(otherFormLine(form("f", "gitti", { tense: "pst", person: "3" })), "gitti (person 3, tense pst)", "features by name, whatever order they came in");
  assert.equal(otherFormLine(form("f", "git", {})), "git (no features)");
});

test("a cell's focus key ignores feature order", () => {
  assert.equal(cellKey("lex_1", { number: "sg", person: "1" }, 0), cellKey("lex_1", { person: "1", number: "sg" }, 0));
  assert.notEqual(cellKey("lex_1", { person: "1", number: "sg" }, 0), cellKey("lex_1", { person: "1", number: "sg" }, 1));
});
