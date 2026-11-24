import { test } from "node:test";
import assert from "node:assert/strict";
import { isPronoun, personNumber, pronounTable } from "./pronouns.ts";

test("pronoun categories", () => {
  for (const c of ["pro", "PRON", "pronoun", "Pronoun", "pers. pron.", "personal pronoun"]) assert.ok(isPronoun(c), c);
  for (const c of ["prep", "proper noun", "n", "", "prop"]) assert.ok(!isPronoun(c), c);
});

test("person and number from a gloss", () => {
  assert.deepEqual(personNumber("1SG"), { person: "1", number: "SG" });
  assert.deepEqual(personNumber("3pl"), { person: "3", number: "PL" });
  assert.deepEqual(personNumber("1PL.INCL"), { person: "1in", number: "PL" });
  assert.deepEqual(personNumber("1du.excl"), { person: "1ex", number: "DU" });
  assert.deepEqual(personNumber("2.SG"), { person: "2", number: "SG" });
  assert.deepEqual(personNumber("I"), { person: "1", number: "SG" });
  assert.deepEqual(personNumber("they; them"), { person: "3", number: "PL" });
  assert.deepEqual(personNumber("you"), { person: "2", number: "" });
  assert.equal(personNumber("this"), null);
  assert.equal(personNumber(""), null);
});

test("the table holds only the persons and numbers met", () => {
  const t = pronounTable([
    { lemma: "na", category: "pro", gloss: "1SG" },
    { lemma: "ŋa", category: "pro", gloss: "1PL.INCL" },
    { lemma: "mi", category: "pro", gloss: "1PL.EXCL" },
    { lemma: "ka", category: "pro", gloss: "2SG" },
    { lemma: "ia", category: "pro", gloss: "3SG" },
    { lemma: "ira", category: "PRON", gloss: "3PL" },
    { lemma: "ira2", category: "pro", gloss: "they" },
    { lemma: "ne", category: "pro", gloss: "this" },
    { lemma: "kesi", category: "n", gloss: "1SG" },
  ])!;
  assert.deepEqual(t.rows, ["1", "1 inclusive", "1 exclusive", "2", "3"]);
  assert.deepEqual(t.columns, ["Singular", "Plural"]);
  assert.deepEqual(t.cells[0], [["na"], []]);
  assert.deepEqual(t.cells[4], [["ia"], ["ira", "ira2"]]);
  assert.deepEqual(t.unplaced, ["ne ‘this’"]);
  assert.equal(pronounTable([{ lemma: "kesi", category: "n", gloss: "dog" }]), null);
});
