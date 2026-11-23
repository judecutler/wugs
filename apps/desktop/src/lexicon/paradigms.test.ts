import assert from "node:assert/strict";
import { test } from "node:test";
import { attestedForms, attestedLine, axesOf, cellFeatures, cellKey, cellLabel, coverageLine, draftOf, featureName, isOwn, layout, otherFormLine, parseValues, templateById, templateFromDraft, templateId, TEMPLATES } from "./paradigms.ts";

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

test("an own template is found beside the built-in ones and marked as own", () => {
  const own = templateFromDraft({ name: "Case × number", rows: "Case", rowValues: "nom=nominative, acc, gen=genitive", cols: "Number", colValues: "sg, pl" }, null, []);
  assert.ok(typeof own !== "string");
  assert.equal(own.id, "case-number");
  assert.deepEqual(own.rows.values, [
    { value: "nom", label: "nominative" },
    { value: "acc", label: "acc" },
    { value: "gen", label: "genitive" },
  ]);
  assert.equal(own.rows.feature, "case");
  assert.equal(templateById("case-number", [own]).label, "Case × number");
  assert.equal(templateById("nothing", [own]).id, "person-number");
  assert.equal(isOwn(own), true);
  assert.equal(isOwn(TEMPLATES[0]!), false);
  assert.deepEqual(draftOf(own), { name: "Case × number", rows: "Case", rowValues: "nom=nominative, acc, gen=genitive", cols: "Number", colValues: "sg, pl" });
});

test("a template form says in words what is missing", () => {
  const d = { name: "Classes", rows: "Noun class", rowValues: "1, 2; 3\n4", cols: "", colValues: "" };
  const t = templateFromDraft(d, null, ["classes"]);
  assert.ok(typeof t !== "string");
  assert.equal(t.id, "classes-2", "a taken id is not reused");
  assert.equal(t.rows.feature, "noun-class");
  assert.equal(t.cols, null);
  assert.equal(templateFromDraft({ ...d, name: " " }, null, []), "Give the template a name.");
  assert.match(templateFromDraft({ ...d, rowValues: "" }, null, []) as string, /List the rows/);
  assert.match(templateFromDraft({ ...d, rowValues: "1, 1" }, null, []) as string, /twice/);
  assert.match(templateFromDraft({ ...d, cols: "noun class", colValues: "a" }, null, []) as string, /two different features/);
  assert.match(templateFromDraft({ ...d, cols: "Number" }, null, []) as string, /List the columns/);
  assert.equal((templateFromDraft(d, "kept", []) as { id: string }).id, "kept");
});

test("feature names are what the engine takes", () => {
  assert.equal(featureName("  Noun Class "), "noun-class");
  assert.equal(featureName("Évidentialité"), "evidentialite");
  assert.equal(featureName("***"), "");
  assert.deepEqual(parseValues("a=, b = bee ,, "), [
    { value: "a", label: "a" },
    { value: "b", label: "bee" },
  ]);
  assert.equal(templateId("***", []), "template");
});

test("an entry's forms in the texts", () => {
  const l = (words: string, at: number, breakdown: string | null, glosses: string | null) => ({ words: words.split(" "), at, breakdown, glosses });
  const lines = [l("nikani ŋai", 0, "ni-kani", "1SG-count"), l("Nikani ra", 0, "ni-kani", "1SG-count"), l("ta kani", 1, null, null), l("kanira", 0, "kani-ra", "count-PL")];
  const forms = attestedForms(lines, ["kani", "kanira"]);
  assert.deepEqual(forms.map((f) => [f.form, f.count, f.inGrid]), [["nikani", 2, false], ["kani", 1, true], ["kanira", 1, true]]);
  assert.equal(attestedLine(forms, 4, 4), "3 forms of this entry in the texts. 1 not in the paradigm yet.");
  assert.equal(attestedLine(forms.slice(1), 2, 9), "2 forms of this entry in the texts (from the first 2 of 9 uses). Every one is in the paradigm.");
  assert.equal(attestedLine([], 0, 0), "No word in the texts is linked to this entry yet. Gloss a few and link them to it.");
});
