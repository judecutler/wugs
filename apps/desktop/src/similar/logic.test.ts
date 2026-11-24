import assert from "node:assert/strict";
import { test } from "node:test";
import { barWidth, similarNote, similarTsv, similarityCell, type Similar } from "./logic.ts";

const r: Similar = { word: "dog", freq: 8, tokens: 112, compared: 9, rows: [{ item: "cat", similarity: 0.934, freq: 8, shared: ["sleeps", "eats"] }] };

test("the note says what was compared and warns on a small corpus", () => {
  const note = similarNote(r, "Dog", 2);
  assert.match(note, /^«dog» occurs 8 times in 112 running words; 9 words were compared by the words within 2 of them\. The word used most like it first\./);
  assert.match(note, /under 5,000 running words/);
  assert.equal(similarNote({ ...r, freq: 0 }, "horse", 2), "«horse» does not occur here.");
  assert.match(similarNote({ ...r, rows: [], tokens: 9000 }, "dog", 2), /No word shares its contexts\.$/);
});

test("the table and cells", () => {
  assert.equal(similarTsv(r), "word\tsimilarity\tfrequency\tshared contexts\ncat\t0.93\t8\tsleeps, eats\n");
  assert.equal(similarityCell(1), "1.00");
  assert.equal(barWidth(0.934), 93);
  assert.equal(barWidth(1.2), 100);
});
