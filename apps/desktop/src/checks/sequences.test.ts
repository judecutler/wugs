import assert from "node:assert/strict";
import { test } from "node:test";
import { MIN_RUNNING, sequenceWords, unusualLine, unusualWords } from "./sequences.ts";

// A small CV language: ta ka pa na ma with i, a, u; then a few odd words.
const base = ["taka", "kapa", "nama", "pata", "mana", "tiku", "kuna", "pita", "mika", "nupa", "tuma", "kani"].map((word) => ({ word, count: 50 }));

test("a rare word with a cluster no other word has is listed", () => {
  const r = unusualWords([...base, { word: "Tkapa", count: 1 }, { word: "kapat", count: 1 }, { word: "makana", count: 40 }]);
  assert.equal(r.running, 12 * 50 + 42);
  const words = r.words.map((w) => w.word);
  assert.ok(words.includes("tkapa"), JSON.stringify(r.words));
  assert.deepEqual(r.words.find((w) => w.word === "tkapa")!.sequences, ["tk"]);
  assert.deepEqual(r.words.find((w) => w.word === "kapat")!.sequences, ["t#"]);
  // A frequent word is not a slip, whatever its sounds.
  assert.ok(!words.includes("makana"));
});

test("a sequence shared by two words is not rare", () => {
  const r = unusualWords([...base, { word: "tkapa", count: 1 }, { word: "tkuna", count: 1 }]);
  assert.ok(r.words.every((w) => !w.sequences.includes("tk")), JSON.stringify(r.words));
});

test("sequences with a rare sound say little and are left out", () => {
  const r = unusualWords([...base, { word: "kaxa", count: 1 }]);
  assert.deepEqual(r.words, []);
});

test("too few words gives nothing, and the lines say why", () => {
  const r = unusualWords([{ word: "tkapa", count: 3 }]);
  assert.deepEqual(r, { words: [], running: 3 });
  assert.match(unusualLine(r), new RegExp(`of at least ${MIN_RUNNING}`));
  assert.match(unusualLine({ words: [], running: 900 }), /Every rare word/);
  assert.match(unusualLine({ words: [{ word: "x", count: 1, sequences: ["tk"], score: 50 }], running: 900 }), /^1 rare word has/);
  assert.equal(sequenceWords("#ŋ"), "ŋ at the start of a word");
  assert.equal(sequenceWords("t#"), "t at the end of a word");
  assert.equal(sequenceWords("tk"), "tk");
});
