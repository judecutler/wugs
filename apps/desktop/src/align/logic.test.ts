import { test } from "node:test";
import assert from "node:assert/strict";
import { alignLine, pairNote, toWrite, writtenLine, type Alignment, type Pair } from "./logic.ts";

const pair = (source: string[], translation: string[], hasOne = false): Pair => ({ segment: source.length > 0 ? "s" + source[0] : null, source, translation, hasOne });

test("a pair's shape is said only when it is not one to one", () => {
  assert.equal(pairNote(pair(["a"], ["A"])), null);
  assert.equal(pairNote(pair(["a", "b"], ["AB"])), "one translation for two sentences: it goes on the first");
  assert.equal(pairNote(pair([], ["X"])), "matched no sentence");
  assert.equal(pairNote(pair(["a"], [])), "no translation found");
});

test("the summary counts each kind and what a write adds", () => {
  const a: Alignment = { pairs: [pair(["a"], ["A"]), pair(["b", "c"], ["BC"]), pair(["d"], [], false), pair([], ["X"]), pair(["e"], ["E"], true)], sources: 5, translations: 4, oneToOne: 2, unmatched: 1, already: 1, written: 0 };
  assert.equal(alignLine(a), "5 sentences and 4 translation sentences: 2 paired one to one; 1 joined; 1 sentence with no translation found; 1 translation sentence matched nothing; 1 already translated and left as they are.");
  assert.equal(toWrite(a), 2);
  assert.equal(writtenLine({ ...a, written: 2 }, "Story (English)"), "Wrote 2 translations from «Story (English)». That text is unchanged; delete it if you no longer need it.");
  assert.match(writtenLine(a, "x"), /nothing was written/);
});
