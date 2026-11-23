import { test } from "node:test";
import assert from "node:assert/strict";
import { checkLine, convertedLines, defaultColumn, defaultSource, defaultTarget, fromGrid, previewNote, toGrid, unknownLine } from "./logic.ts";

test("the column, source and target are picked sensibly", () => {
  assert.equal(defaultColumn(["Orthography", "ipa"]), "ipa");
  assert.equal(defaultColumn(["X-SAMPA"]), "X-SAMPA");
  assert.equal(defaultColumn([]), null);
  assert.equal(defaultTarget("IPA", "utterance"), "IPA");
  assert.equal(defaultTarget("IPA", "ipa"), "IPA from ipa");
  assert.equal(defaultTarget("IPA", "utterance", ["IPA", "utterance"]), "IPA from utterance");
  assert.equal(defaultTarget("IPA", "utterance", ["IPA", "IPA from utterance"]), "IPA from utterance (2)");
  const tracks = [
    { id: "t1", name: "IPA", segments: 40 },
    { id: "t2", name: "utterance", segments: 30 },
  ];
  assert.equal(defaultSource(tracks), "t2");
  assert.equal(defaultSource([tracks[0]!]), "t1");
  assert.equal(defaultSource([]), null);
});

test("what a profile holds and what it misses is said in words", () => {
  assert.equal(checkLine({ columns: [], rules: 0, problems: [] }), "No profile yet.");
  assert.equal(checkLine({ columns: ["IPA"], rules: 1, problems: [] }), "1 row, converting into IPA.");
  assert.equal(unknownLine([]), "");
  const line = unknownLine([{ text: "z", count: 3, example: "jaz" }, { text: "q", count: 1, example: "qa" }], 1);
  assert.match(line, /«z» 3× \(in «jaz»\) and 1 more/);
  assert.equal(previewNote({ check: { columns: [], rules: 0, problems: [] }, segments: 0, changes: [], unknown: [] }), "This track has no segments to convert.");
  assert.equal(previewNote({ check: { columns: [], rules: 0, problems: [] }, segments: 50, changes: [{ before: "a", after: "a" }], unknown: [] }), "50 segments on this track. The first 1 are shown.");
});

test("a conversion's report says what was written and what was kept", () => {
  const lines = convertedLines({ track: "t", trackName: "IPA", texts: 2, created: 5, updated: 1, unchanged: 0, kept: 1, empty: 0, unknown: [{ text: "z", count: 1, example: "z" }] });
  assert.deepEqual(lines, ["The «IPA» track in 2 texts: 5 new, 1 rewritten.", "1 segment was edited by hand since the last conversion and kept as it is.", "1 letter had no row and stayed as written: «z»"]);
  assert.equal(convertedLines({ track: "t", trackName: "IPA", texts: 1, created: 0, updated: 0, unchanged: 0, kept: 0, empty: 2, unknown: [] })[0], "The «IPA» track in 1 text: nothing to write.");
});

test("a profile goes to a grid and back, comments and columns kept", () => {
  const text = "# Kanu practical spelling\nGrapheme\tIPA\tComment\nsh\tʃ\nng\tŋ\tvelar\n\n";
  const g = toGrid(text);
  assert.deepEqual(g, { comments: ["# Kanu practical spelling"], header: ["Grapheme", "IPA", "Comment"], rows: [["sh", "ʃ", ""], ["ng", "ŋ", "velar"]] });
  g.rows.push(["", "", ""], ["a\tb", "x", ""]);
  assert.equal(fromGrid(g), "# Kanu practical spelling\nGrapheme\tIPA\tComment\nsh\tʃ\t\nng\tŋ\tvelar\na b\tx\t\n");
  assert.deepEqual(toGrid("").header, ["Grapheme", "IPA"]);
});
