import assert from "node:assert/strict";
import { test } from "node:test";
import { taggedLines } from "./logic.ts";

test("a run says what was tagged, kept and left alone", () => {
  const lines = taggedLines({ annotated: 3, tokenized: 1, kept: 2, differ: 1, words: 17, by: "udpipe english.udpipe" }, "Story");
  assert.equal(lines[0], "Tagged and parsed 3 sentences of «Story» (17 words) with udpipe english.udpipe.");
  assert.equal(lines[1], "1 sentence was broken into words the tagger's way.");
  assert.equal(lines[2], "2 sentences keep their own tags.");
  assert.match(lines[3]!, /^1 sentence was left alone: its words are split differently/);
});

test("nothing to do is said plainly", () => {
  assert.deepEqual(taggedLines({ annotated: 0, tokenized: 0, kept: 4, differ: 0, words: 0, by: "" }, "Story"), ["Every sentence of «Story» has tags already; nothing was sent."]);
  assert.deepEqual(taggedLines({ annotated: 0, tokenized: 0, kept: 0, differ: 0, words: 0, by: "" }, "Story"), ["«Story» has no transcribed sentences to tag."]);
});
