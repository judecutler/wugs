import assert from "node:assert/strict";
import { test } from "node:test";
import { importLines, keyOf, label, queryHint, suggestions } from "./logic.ts";

test("keys are made as the engine makes them", () => {
  assert.equal(keyOf("Text Genre"), "text_genre");
  assert.equal(keyOf("  dc:date "), "dc_date");
  assert.equal(keyOf("Автор"), "автор");
  assert.equal(keyOf("लेखक का नाम"), "लेखक_का_नाम", "vowel signs are part of a word, as Rust's is_alphanumeric has it");
  assert.equal(keyOf("__speaker__"), "speaker");
  assert.equal(keyOf("1st"), null);
  assert.equal(keyOf("--"), null);
  assert.equal(keyOf("a".repeat(60))?.length, 40);
});

test("labels read as words", () => {
  assert.equal(label("text_genre"), "Text genre");
});

test("suggestions put the project's own keys first and skip the text's", () => {
  const s = suggestions({ fields: [{ key: "author", value: "A" }], keys: [{ key: "author", texts: 3 }, { key: "year", texts: 5 }, { key: "dialect", texts: 1 }] });
  assert.deepEqual(s.slice(0, 3), ["year", "dialect", "date"]);
  assert.ok(!s.includes("author"));
});

test("a query hint escapes what a regular expression would read", () => {
  assert.equal(queryHint("genre", 'news (wire) "x"'), ':: match.text_genre="news \\(wire\\) \\"x\\""');
});

test("an import report says what matched and what did not", () => {
  const lines = importLines({ by: "File", keys: ["genre", "year"], texts: 2, fields: 3, unmatched: ["a", "b", "c", "d", "e", "f"], unmatchedCount: 7, shared: 1 });
  assert.equal(lines[0], "3 values for 2 texts, matched by «File».");
  assert.equal(lines[2], "7 rows name no text here: «a», «b», «c», «d», «e» and 2 more.");
  assert.match(lines[3], /several texts share/);
  assert.equal(importLines({ by: "title", keys: ["genre"], texts: 0, fields: 0, unmatched: [], unmatchedCount: 0, shared: 0 })[0], "Every text already had these values; nothing changed.");
});
