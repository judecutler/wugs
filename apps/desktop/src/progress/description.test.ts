import { test } from "node:test";
import assert from "node:assert/strict";
import { corpusDescription, describedKeys } from "./description.ts";
import type { Progress } from "./logic.ts";

const text = (title: string, meta: [string, string][]) => ({ id: title, title, recordingMs: 60_000, sentences: 10, transcribed: 8, transcribedMs: 30_000, words: 50, glossedWords: 25, morphs: 40, translated: 4, meta });
const progress: Progress = { texts: [text("A", [["genre", "narrative"]]), text("B", [["genre", "song"], ["place", "Lamba"]])], recordedMs: 120_000, recordingsMissing: 0, entries: 30, senses: 34, sensesWithoutGloss: 2, people: 2, consentRecords: 1 };

test("the description sums the corpus and leaves places to write", () => {
  assert.deepEqual(describedKeys(progress), ["genre", "place"]);
  const html = corpusDescription({ progress, speakers: [{ tier: "t", name: "MAR", texts: 2, utterances: 16, words: 100, types: 40, longest: 9, analysed: 0, morphemes: 0, mtld: null, vocd: null, person: "", sex: "", age: "", born: "", byText: [] }], language: { name: "Kanu", glottocode: "abcd1234", iso: "" }, restricted: 1, uncovered: 0, date: "2026-11-24", generator: "wugs test" });
  assert.match(html, /<title>Kanu: description of the collection<\/title>/);
  assert.ok(html.includes("Kanu (Glottocode abcd1234)"));
  assert.ok(html.includes("<tr><th scope=\"row\">Transcribed</th><td>16 of 20 sentences (80%)"));
  assert.ok(html.includes("<h3>By genre</h3>") && html.includes("<td>narrative</td>"));
  assert.ok(html.includes("<td>MAR</td>"));
  assert.ok(html.includes("1 of 2 texts have a sharing level other than open") && html.includes("A consent record covers every text."));
  assert.ok((html.match(/class="todo"/g) ?? []).length >= 5);
});

test("the speakers' sex and age go in, never their names", () => {
  const speaker = { tier: "t", name: "CHI", texts: 1, utterances: 4, words: 10, types: 6, longest: 4, analysed: 0, morphemes: 0, mtld: null, vocd: null, person: "Lena Okafor", sex: "female", age: "2;06", born: "", byText: [] };
  const html = corpusDescription({ progress, speakers: [speaker], language: { name: "Kanu", glottocode: "", iso: "" }, restricted: 0, uncovered: null, date: "2026-11-24", generator: "wugs test" });
  assert.ok(html.includes("<th>Sex</th><th>Age</th>") && html.includes("<td>CHI</td><td>female</td><td>2;06</td>"));
  assert.ok(!html.includes("Lena"));
});
