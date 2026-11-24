import { test } from "node:test";
import assert from "node:assert/strict";
import { chunks, classify, dragAction, outcomeLines, overlayWords, progressLine, batchLines } from "./logic.ts";

test("each file takes the import its type needs", () => {
  assert.equal(classify("Interview 3.WAV"), "recording");
  assert.equal(classify("voice memo.m4a"), "recording");
  assert.equal(classify("song.flac"), "recording");
  assert.equal(classify("corpus.CSV"), "file");
  assert.equal(classify("captions.vtt"), "file");
  assert.equal(classify("lunch.cha"), "file");
  assert.equal(classify("stall.exb"), "file");
  assert.equal(classify("Frog story.eaf"), "file");
  assert.equal(classify("en_ewt-ud-test.conllu"), "file");
  assert.equal(classify("words.TextGrid"), "file");
  assert.equal(classify("dictionary.lift"), "file");
  assert.equal(classify("toolbox.txt"), "file");
  assert.equal(classify("notes.docx"), "unsupported");
  assert.equal(classify("README"), "unsupported");
});

test("the overlay says what a drop will do", () => {
  assert.equal(overlayWords(false, { count: 2, audio: 2 }).title, "Open a project first");
  assert.equal(overlayWords(true, { count: 2, audio: 2 }).title, "Drop to import 2 recordings");
  assert.equal(overlayWords(true, { count: 1, audio: 1 }).title, "Drop to import 1 recording");
  assert.equal(overlayWords(true, { count: 3, audio: 1 }).title, "Drop to import 3 files");
  assert.equal(overlayWords(true, { count: 0, audio: 0 }).title, "Drop to import");
});

test("a file is cut into pieces that cover it once, in order", () => {
  assert.deepEqual(chunks(0, 4), []);
  assert.deepEqual(chunks(4, 4), [[0, 4]]);
  assert.deepEqual(chunks(10, 4), [[0, 4], [4, 8], [8, 10]]);
  const big = chunks(2.5e9);
  assert.equal(big.at(-1)?.[1], 2.5e9);
  assert.ok(big.every(([a, b], i) => b - a <= 8 * 1024 * 1024 && (i === 0 ? a === 0 : a === big[i - 1]![1])));
});

test("progress and results read in words", () => {
  assert.equal(progressLine("a.wav", 1.2e9, 2.4e9, 0), "Copying «a.wav» into the project: 50% of 2.4 GB");
  assert.equal(progressLine("a.wav", 10, 10, 2), "Importing «a.wav»… (2 more files after this)");
  assert.deepEqual(outcomeLines("a.eaf", { kind: "file", format: "ELAN", lines: ["A new text was made."], text: "t1", lexicon: false }), ["Imported «a.eaf» (ELAN).", "A new text was made."]);
  assert.deepEqual(outcomeLines("x.docx", { kind: "refused", reason: "no." }), ["«x.docx» was not imported: no."]);
});

test("only files are imported; dock tabs and text fields keep their drags; anything else is refused", () => {
  assert.equal(dragAction({ files: true, dock: false, editable: false }), "import");
  assert.equal(dragAction({ files: true, dock: false, editable: true }), "import");
  assert.equal(dragAction({ files: false, dock: true, editable: false }), "pass");
  assert.equal(dragAction({ files: false, dock: false, editable: true }), "pass");
  assert.equal(dragAction({ files: false, dock: false, editable: false }), "refuse", "a link from a browser never reaches the webview");
  assert.equal(dragAction({ files: false, dock: false, editable: false, card: true }), "pass", "a media bin card goes to the Timeline");
});

test("several files imported at once are reported in one toast", () => {
  const lines = batchLines([
    { name: "a.eaf", format: "ELAN", error: null },
    { name: "b.eaf", format: "ELAN", error: null },
    { name: "c.txt", format: "plain text", error: null },
    { name: "d.doc", format: null, error: "not a format wugs reads" },
  ]);
  assert.deepEqual(lines, ["Imported 3 of 4 files: 2 ELAN, 1 plain text.", "«d.doc» was not imported: not a format wugs reads"]);
  const many = batchLines(Array.from({ length: 8 }, (_, i) => ({ name: `${i}.x`, format: null, error: "no" })));
  assert.equal(many[0], "Imported 0 of 8 files.");
  assert.equal(many.at(-1), "3 more files were not imported.");
});

test("exporting every text reports the files and the failures", async () => {
  const { exportedAllLines } = await import("./logic.ts");
  assert.deepEqual(exportedAllLines(3, 3, "/out", [], 0), ["Saved 3 texts of 3 in /out."]);
  assert.deepEqual(exportedAllLines(1, 2, "/out", [{ title: "", error: "no timed sentences" }], 1), ["Saved 1 text of 2 in /out.", "1 export has notes; export a text alone to read them.", "«untitled»: no timed sentences"]);
});

test("a SayMore project's import is reported in words", async () => {
  const { saymoreLines } = await import("./logic.ts");
  assert.deepEqual(saymoreLines({ sessions: 3, transcribed: 2, empty: 1, recordings: 3, details: 12, failed: [["ETR011", "no ELAN"]], text: "t" }), [
    "Read 3 sessions: 2 transcriptions from their ELAN files, and 1 new empty text for sessions not transcribed yet.",
    "3 recordings were imported and linked, so the texts open with their sound.",
    "12 details (date, place, genre, speakers…) written from the session files; see them under the text's Details.",
    "ETR011: no ELAN",
  ]);
});
