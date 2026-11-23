import assert from "node:assert/strict";
import { test } from "node:test";
import { composition, compositionNote, compositionTsv, detailKeys, keyLabel, NONE, wordShare } from "./composition.ts";
import type { TextProgress } from "./logic.ts";

const t = (title: string, words: number, meta: [string, string][]): TextProgress => ({
  id: title, title, recordingMs: null, sentences: 5, transcribed: 4, transcribedMs: 0, words, glossedWords: words / 2, morphs: words, translated: 0, meta,
});
const texts = [
  t("A", 100, [["genre", "narrative"], ["speaker", "M"]]),
  t("B", 300, [["genre", "procedural"]]),
  t("C", 50, [["genre", "narrative"]]),
  t("D", 10, []),
];

test("keys and rows", () => {
  assert.deepEqual(detailKeys(texts), [{ key: "genre", texts: 3 }, { key: "speaker", texts: 1 }]);
  const rows = composition(texts, "genre");
  assert.deepEqual(rows.map((r) => [r.value, r.texts, r.words, r.sentences]), [["procedural", 1, 300, 4], ["narrative", 2, 150, 8], [NONE, 1, 10, 4]]);
  assert.equal(wordShare(rows[0]!, rows), "65%");
  assert.equal(keyLabel("recording_place"), "recording place");
});

test("note and copy", () => {
  const rows = composition(texts, "genre");
  assert.equal(compositionNote(rows, "genre"), "2 values of genre. 1 text does not give it: add it in the Texts panel's Details or with Import details….");
  assert.equal(compositionNote(composition(texts.slice(0, 3), "genre"), "genre"), "2 values of genre.");
  assert.equal(compositionTsv(rows, "genre").split("\n")[1], "procedural\t1\t4\t300\t150");
  assert.equal(compositionTsv(rows, "genre").split("\n")[4], "All texts\t4\t16\t460\t230");
});
