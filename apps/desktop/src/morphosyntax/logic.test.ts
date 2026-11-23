import assert from "node:assert/strict";
import { test } from "node:test";
import { setupFor, spacyNote, taggedLines } from "./logic.ts";

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

test("the setup rows follow what was chosen, and Automatic what it found", () => {
  assert.equal(setupFor({ machine: "udpipe-lindat", pin: null, effective: "udpipe-lindat" }), "web");
  assert.equal(setupFor({ machine: "automatic", pin: "spacy", effective: null }), "spacy", "the pin wins");
  assert.equal(setupFor({ machine: "udpipe", pin: null, effective: null }), "udpipe");
  assert.equal(setupFor({ machine: "automatic", pin: null, effective: "spacy" }), "spacy");
  assert.equal(setupFor({ machine: "automatic", pin: null, effective: null }), "local");
  assert.match(spacyNote({ pipeline: "en_core_web_sm", about: "spaCy 3.8, en_core_web_sm 3.8.0 (parser)" }), /^Checked: spaCy 3\.8/u);
  assert.match(spacyNote(null), /pip install spacy/u);
});
