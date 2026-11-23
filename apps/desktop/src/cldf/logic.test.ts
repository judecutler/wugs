import { test } from "node:test";
import assert from "node:assert/strict";
import { glottocodeProblem, isoProblem, publishedLines, corpusLines, dictionaryLines, siteLines } from "./logic.ts";

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
  const clipped = publishedLines({ path: "/x", entries: 1, senses: 1, examples: 3, clips: 2, restrictedLeftOut: 0, notes: [] });
  assert.equal(clipped[2], "2 sentences come with their sound clip, cut from the recording, in the audio folder.");
  const withExamples = publishedLines({ path: "/x", entries: 1, senses: 1, examples: 3, lexiconExamples: 1, restrictedLeftOut: 0, notes: [] });
  assert.match(withExamples[1]!, /3 glossed sentences, with 1 example sentence from the lexicon, ready/u);
});

test("a website export says what it holds and what was left out", () => {
  const lines = siteLines({ path: "/x/wugs-site-2026-11-23", texts: 2, sentences: 31, entries: 120, clips: 30, restrictedLeftOut: 1, clipsMissing: 1 });
  assert.match(lines[0]!, /^Saved a website in \/x\/wugs-site-2026-11-23: 2 texts \(31 sentences\) and a dictionary of 120 entries\./u);
  assert.equal(lines[1], "30 sentences play from their recording.");
  assert.match(lines[2]!, /^1 timed sentence has no sound/u);
  assert.equal(lines[3], "1 text that is not open was left out.");
  assert.match(siteLines({ path: "/x", texts: 0, sentences: 0, entries: 4, clips: 0, restrictedLeftOut: 0, clipsMissing: 0 })[0]!, /: a dictionary of 4 entries\./u);
});

test("a corpus export says where it went and what was left out", () => {
  const lines = corpusLines({ path: "/x/wugs-cwb-2026-11-23", corpus: "KAN_WUGS", texts: 3, sentences: 41, restrictedLeftOut: 1, notes: ["n"] });
  assert.equal(lines[0], "Saved the corpus KAN_WUGS in /x/wugs-cwb-2026-11-23: 3 texts, 41 sentences.");
  assert.equal(lines[2], "1 text that is not open was left out.");
  assert.equal(lines.at(-1), "n");
});

test("a dictionary export says how to print it", () => {
  const lines = dictionaryLines({ path: "/x/Kanu dictionary.html", entries: 120, senses: 150, examples: 1, restrictedLeftOut: 2 });
  assert.equal(lines[0], "Saved the dictionary as /x/Kanu dictionary.html: 120 entries, 150 senses, 1 example from the texts.");
  assert.equal(lines[2], "2 examples were left out because their texts are not open.");
});
