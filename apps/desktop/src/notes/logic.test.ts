import { test } from "node:test";
import assert from "node:assert/strict";
import { entryPlace, headwordOf, isQuestion, notesLine, notesSheet, notesText, shown, type Notes } from "./logic.ts";

const notes: Notes = {
  sentences: [
    { noteId: "n1", body: "Plural?", textId: "t1", textTitle: "Ants", tierId: "u", segmentId: "s1", content: "tik tak" },
    { noteId: "n2", body: "Speaker laughed", textId: "t1", textTitle: "Ants", tierId: "u", segmentId: "s2", content: "hó" },
    { noteId: "n3", body: "Ask about the tone", textId: "t2", textTitle: "Bees", tierId: "u", segmentId: "s3", content: "zum" },
  ],
  entries: [
    { entryId: "e1", headword: "tela", homograph: 2, sense: null, gloss: "cloth", body: "Loan from Spanish?" },
    { entryId: "e1", headword: "tela", homograph: 2, sense: 1, gloss: "cloth", body: "Only woven" },
  ],
};

test("questions and filters", () => {
  assert.ok(isQuestion("Plural?"));
  assert.ok(isQuestion("¿cuál"));
  assert.ok(!isQuestion("Speaker laughed"));
  const q = shown(notes, { which: "all", query: "", questions: true });
  assert.deepEqual(q.sentences.map((s) => s.noteId), ["n1"]);
  assert.equal(q.entries.length, 1);
  assert.equal(shown(notes, { which: "sentences", query: "", questions: false }).entries.length, 0);
  assert.equal(shown(notes, { which: "entries", query: "", questions: false }).sentences.length, 0);
  // The query looks in the sentence too, ignoring accents.
  assert.deepEqual(shown(notes, { which: "all", query: "HO", questions: false }).sentences.map((s) => s.noteId), ["n2"]);
  assert.deepEqual(shown(notes, { which: "all", query: "cloth", questions: false }).entries.length, 2);
});

test("lines and places", () => {
  assert.match(notesLine({ sentences: [], entries: [] }, { sentences: [], entries: [] }), /^No notes yet/);
  assert.equal(notesLine(notes, notes), "3 notes on sentences and 2 notes in the lexicon, 2 of them questions.");
  const one = shown(notes, { which: "all", query: "tone", questions: false });
  assert.equal(notesLine(notes, one), "3 notes on sentences and 2 notes in the lexicon, 2 of them questions; 1 shown.");
  assert.equal(headwordOf(notes.entries[0]!), "tela₂");
  assert.equal(entryPlace(notes.entries[0]!), "tela₂ ‘cloth’");
  assert.equal(entryPlace(notes.entries[1]!), "tela₂, sense 1 ‘cloth’");
});

test("the list as text and as a sheet", () => {
  assert.equal(
    notesText(notes),
    "Ants\n- tik tak\n  Plural?\n- hó\n  Speaker laughed\n\nBees\n- zum\n  Ask about the tone\n\nLexicon\n- tela₂ ‘cloth’\n  Loan from Spanish?\n- tela₂, sense 1 ‘cloth’\n  Only woven\n",
  );
  assert.equal(notesText({ sentences: [], entries: [] }), "");
  const html = notesSheet({ sentences: [{ ...notes.sentences[0]!, content: "a<b" }], entries: [] }, { language: "Tikal", date: "2026-11-23", generator: "wugs" });
  assert.ok(html.includes("<h1>Tikal: notes to go through</h1>"));
  assert.ok(html.includes("<h2>Ants</h2>"));
  assert.ok(html.includes("a&lt;b"));
  assert.ok(!html.includes("<h2>Lexicon</h2>"));
});
