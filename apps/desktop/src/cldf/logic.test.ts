import { test } from "node:test";
import assert from "node:assert/strict";
import { glottocodeProblem, isoProblem, publishedLines, corpusLines, dictionaryLines, flashcardLines, siteLines } from "./logic.ts";
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
  assert.equal(lines[2], "2 examples were left out because their texts are not open.");
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
  assert.match(flashcardLines({ path: "/x", cards: 1, sounds: 0, noGloss: 0 })[1]!, /^No card has a sound yet/u);
});
