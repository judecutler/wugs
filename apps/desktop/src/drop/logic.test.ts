import { test } from "node:test";
import assert from "node:assert/strict";
import { chunks, classify, dragAction, outcomeLines, overlayWords, progressLine } from "./logic.ts";

test("each file takes the import its type needs", () => {
  assert.equal(classify("Interview 3.WAV"), "recording");
  assert.equal(classify("voice memo.m4a"), "recording");
  assert.equal(classify("song.flac"), "recording");
  assert.equal(classify("corpus.CSV"), "file");
  assert.equal(classify("Frog story.eaf"), "file");
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
