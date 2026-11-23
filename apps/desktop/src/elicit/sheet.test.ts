import { test } from "node:test";
import assert from "node:assert/strict";
import { elicitationSheet, sheetRows, type SheetOptions } from "./sheet.ts";

const base: SheetOptions = {
  list: { name: "Swadesh 100", citation: "Swadesh 1971" },
  language: "Kanu",
  concepts: [
    { number: 2, gloss: "you", concepticonGloss: "THOU", skipped: false, answers: [{ lemma: "ka", gloss: "you" }] },
    { number: 1, gloss: "I", concepticonGloss: "I", skipped: false, answers: [] },
    { number: 3, gloss: "we", concepticonGloss: "WE", skipped: true, answers: [] },
    { number: 4, gloss: "<this>", concepticonGloss: "", skipped: false, answers: [] },
  ],
  which: "all",
  withAnswers: true,
  generator: "wugs",
};

test("the sheet's rows are the list in order, or what is still to ask", () => {
  assert.deepEqual(sheetRows(base).map((c) => c.number), [1, 2, 3, 4]);
  assert.deepEqual(sheetRows({ ...base, which: "todo" }).map((c) => c.number), [1, 4]);
});

test("the sheet prints a header to fill in, the concepts, and answers to check", () => {
  const html = elicitationSheet(base);
  assert.match(html, /<h1>Swadesh 100 — Kanu<\/h1>/);
  assert.match(html, /Swadesh 1971/);
  assert.match(html, /<span>Speaker<\/span>/);
  assert.match(html, /<td class="n">2<\/td><td class="g">you<div class="cg">THOU<\/div><\/td><td class="w"><span class="had">ka<\/span><\/td>/);
  assert.doesNotMatch(html, /<td class="g">I<div/, "the same gloss is not printed twice");
  assert.match(html, /set aside/);
  assert.match(html, /&lt;this&gt;/);
  assert.match(html, /4 concepts\./);
  const blank = elicitationSheet({ ...base, withAnswers: false, which: "todo" });
  assert.doesNotMatch(blank, /class="had"/);
  assert.match(blank, /2 concepts\./);
});
