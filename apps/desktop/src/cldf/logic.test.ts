import { test } from "node:test";
import assert from "node:assert/strict";
import { glottocodeProblem, isoProblem, publishedLines, corpusLines, dictionaryLines, flashcardLines, sheetLines, siteLines } from "./logic.ts";
import { languoidLine, pickedLanguage, worthLooking } from "./logic.ts";

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
  assert.ok(siteLines({ path: "/x", texts: 1, sentences: 3, entries: 0, clips: 0, restrictedLeftOut: 0, clipsMissing: 0, pages: 2 }).includes("Each line marked on a page image shows its piece of the page, from 2 pages in the pages folder."));
  const said = siteLines({ path: "/x", texts: 1, sentences: 3, entries: 4, clips: 5, restrictedLeftOut: 0, clipsMissing: 0, headwordClips: 2 });
  assert.deepEqual(said.slice(1, 3), ["3 sentences play from their recording.", "2 dictionary headwords play as they are said."]);
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
  assert.equal(lines[2], "2 examples were left out because their texts are not open.");  assert.equal(dictionaryLines({ path: "/x", entries: 1, senses: 1, examples: 0, restrictedLeftOut: 0, topics: 12 })[2], "Words by topic lists the entries under 12 semantic domains.");
});

test("a Glottolog match reads as what and where it is", () => {
  const tr = { glottocode: "nucl1301", iso: "tur", name: "Turkish", dialect: false, family: "Turkic", macroarea: "Eurasia", language: "" };
  assert.equal(languoidLine(tr), "Turkish — tur · nucl1301; Turkic, Eurasia");
  assert.equal(languoidLine({ ...tr, name: "Ainu", glottocode: "ainu1240", iso: "", family: "isolate" }), "Ainu — ainu1240; an isolate, Eurasia");
  assert.equal(languoidLine({ ...tr, name: "Cypriot Turkish", glottocode: "cypr1248", iso: "", dialect: true, language: "Turkish" }), "Cypriot Turkish — cypr1248; a dialect of Turkish, Eurasia");
  assert.equal(languoidLine({ ...tr, macroarea: "Africa;Eurasia" }), "Turkish — tur · nucl1301; Turkic, Africa, Eurasia");
  assert.equal(worthLooking("t"), false);
  assert.equal(worthLooking(" tu "), true);
});

test("picking a match keeps a name the person gave", () => {
  const tr = { glottocode: "nucl1301", iso: "tur", name: "Turkish", dialect: false, family: "Turkic", macroarea: "Eurasia", language: "" };
  assert.deepEqual(pickedLanguage({ name: "", glottocode: "", iso: "" }, tr, "turk"), { name: "Turkish", glottocode: "nucl1301", iso: "tur" });
  assert.deepEqual(pickedLanguage({ name: "Türkçe", glottocode: "", iso: "" }, tr, ""), { name: "Türkçe", glottocode: "nucl1301", iso: "tur" }, "found from the code field");
  assert.deepEqual(pickedLanguage({ name: "Türkçe", glottocode: "", iso: "" }, tr, "turkish"), { name: "Turkish", glottocode: "nucl1301", iso: "tur" }, "typed in the name field: the match's name");
});

test("flashcard lines say what to import and what is missing", () => {
  const lines = flashcardLines({ path: "/x/wugs-flashcards-2026-11-23", cards: 40, sounds: 1, noGloss: 2 });
  assert.match(lines[0]!, /^Saved 40 flashcards in \/x\/wugs-flashcards-2026-11-23: anki\.txt/u);
  assert.equal(lines[1], "1 card plays the word as it is said: copy the sounds folder into Anki's media folder first.");
  assert.equal(lines[2], "2 entries with no gloss were left out.");
  const top = flashcardLines({ path: "/x", cards: 100, sounds: 0, noGloss: 0, used: 60, pastLimit: 312 });
  assert.equal(top[2], "Commonest first: 60 cards are words the glossed texts use, most used first, then 40 cards the texts do not use yet, A to Z.");
  assert.equal(top[3], "312 less common words were left for a later set.");
  assert.match(flashcardLines({ path: "/x", cards: 3, sounds: 0, noGloss: 0, used: 0, pastLimit: 0 })[2]!, /^No word of them is glossed/u);
  assert.match(flashcardLines({ path: "/x", cards: 1, sounds: 0, noGloss: 0 })[1]!, /^No card has a sound yet/u);
});

test("catalogue sheet lines", () => {
  const lines = sheetLines({ path: "/x/a.csv", texts: 3, withoutRecording: 1, notOpen: 2 });
  assert.match(lines[0]!, /^Saved a catalogue sheet of 3 texts in \/x\/a\.csv/u);
  assert.equal(lines[1], "1 text has no recording linked here, so no file name or length.");
  assert.equal(lines[2], "2 texts are not open; the access column says so.");
});

test("the toast after a parallel corpus was saved", async () => {
  const { parallelLines } = await import("./logic.ts");
  const lines = parallelLines({ path: "/x/wugs-parallel-2026-11-24", pairs: 2, texts: 1, languages: [["und", 2]], untranslated: 1, restrictedLeftOut: 3 });
  assert.match(lines[0]!, /^Saved a parallel corpus of 2 sentence pairs from 1 text in \/x\/wugs-parallel-2026-11-24, translated into und \(2 pairs\)/);
  assert.match(lines[1]!, /no language are filed under und/);
  assert.equal(lines[2], "1 transcribed sentence has no translation yet and was left out.");
  assert.equal(lines[3], "3 texts are not open and were left out.");
  assert.equal(parallelLines({ path: "p", pairs: 1, texts: 1, languages: [["eng", 1]], untranslated: 0, restrictedLeftOut: 0 }).length, 1);
});

test("the toast after a spelling dictionary was saved", async () => {
  const { spellingLines } = await import("./logic.ts");
  const lines = spellingLines({ path: "/x/wugs-spelling-2026-11-24", words: 120, fromLexicon: 40, once: 7, code: "und" });
  assert.match(lines[0]!, /^Saved a spelling dictionary of 120 words in \/x\/wugs-spelling-2026-11-24: und\.dic and und\.aff/);
  assert.match(lines[1]!, /ISO 639-3 code/);
  assert.match(lines[2]!, /^7 words met only once/);
  assert.equal(spellingLines({ path: "p", words: 1, fromLexicon: 1, once: 0, code: "kan" }).length, 1);
});

test("sentence cards are reported as sentences", async () => {
  const { flashcardLines } = await import("./logic.ts");
  const lines = flashcardLines({ path: "/x", cards: 12, sounds: 10, noGloss: 3, sentences: true });
  assert.equal(lines[0], "Saved 12 sentence cards in /x: anki.txt for Anki (the sentence asked, its meaning the answer) and quizlet.txt for Quizlet. The README there says how to import them.");
  assert.equal(lines[2], "3 sentences with no translation were left out.");
});

test("a UniMorph export names where its forms came from", async () => {
  const { unimorphLines } = await import("./logic.ts");
  assert.deepEqual(unimorphLines({ path: "/x/u", code: "tur", fromGrids: 2, fromTexts: 30, bare: 5, unknown: ["FOC"] }), [
    "Saved 32 inflected forms in UniMorph's format in /x/u (the file tur): 2 from the paradigm grids, 30 from glossed words in the texts.",
    "Not in UniMorph's schema, written as glossed: FOC. The README says to rename them before sharing.",
  ]);
});
