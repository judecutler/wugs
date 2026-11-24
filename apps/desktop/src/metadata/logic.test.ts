import assert from "node:assert/strict";
import { test } from "node:test";
import { importLines, keyOf, label, parseScope, queryHint, suggestions, trackValue, whereOptions, whereValue, whereWords } from "./logic.ts";

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

test("a scope picker's values round-trip", () => {
  const w = { key: "genre", value: 'news "wire"' };
  assert.deepEqual(parseScope(whereValue(w)), { textId: null, within: w });
  assert.deepEqual(parseScope(""), { textId: null, within: null });
  assert.deepEqual(parseScope("txt_1"), { textId: "txt_1", within: null });
  assert.deepEqual(parseScope("where:{broken"), { textId: null, within: null });
  assert.deepEqual(parseScope(trackValue("tier_1", "CHI")), { textId: null, within: { key: "", value: "", tier: "tier_1", trackName: "CHI" } });
  assert.deepEqual(parseScope("track:[1"), { textId: null, within: null });
});

test("details with a value every text shares are not offered", () => {
  const opts = whereOptions(
    [
      { key: "language", values: [["kanu", 3]] },
      { key: "genre", values: [["news", 2], ["story", 1]] },
    ],
    3,
  );
  assert.deepEqual(opts.map((o) => o.key), ["genre"]);
  assert.equal(opts[0]!.options[0]!.label, "Genre: news (2 texts)");
  assert.equal(whereWords({ key: "text_genre", value: "news" }), "texts whose text genre is «news»");
  assert.equal(whereWords({ key: "", value: "", tier: "t1", trackName: "CHI" }), "the «CHI» track");
});

test("a speaker's detail is a scope of its own", async () => {
  const { parseScope, speakerOptions, speakerValue, whereWords } = await import("./logic.ts");
  const scope = parseScope(speakerValue("sex", "female"));
  assert.deepEqual(scope, { textId: null, within: { key: "sex", value: "female", speaker: true } });
  assert.equal(whereWords(scope.within!), "speakers whose sex is «female»");
  assert.deepEqual(speakerOptions([["sex", [["female", 2], ["male", 1]]]]).map((o) => o.label), ["Sex: female (2 tracks)", "Sex: male (1 track)"]);
});
