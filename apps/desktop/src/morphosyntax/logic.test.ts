import assert from "node:assert/strict";
import { test } from "node:test";
import { addTagged, NO_TAGS, projectTaggedLines, setupFor, shouldStop, spacyNote, stanzaNote, taggedLines } from "./logic.ts";

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
  assert.equal(setupFor({ machine: "stanza", pin: null, effective: null }), "stanza", "an unready choice still shows its rows");
  assert.equal(setupFor({ machine: "automatic", pin: null, effective: "stanza" }), "stanza");
  assert.equal(setupFor({ machine: "automatic", pin: null, effective: null }), "local", "nothing found: every local tagger's rows");
  assert.match(stanzaNote(null), /pip install stanza/u);
  assert.match(stanzaNote({ pipeline: "de", about: "Stanza 1.10.1, de (tokenize, pos)" }), /^Checked: Stanza 1\.10\.1, de/u);
});

test("a run over every text adds up and says what failed", () => {
  const a = { annotated: 3, tokenized: 1, kept: 0, differ: 0, words: 12, by: "udpipe english.udpipe" };
  const b = { annotated: 0, tokenized: 0, kept: 2, differ: 1, words: 0, by: "" };
  const total = addTagged(addTagged(NO_TAGS, a), b);
  assert.deepEqual(total, { annotated: 3, tokenized: 1, kept: 2, differ: 1, words: 12, by: "udpipe english.udpipe" });
  const lines = projectTaggedLines({ texts: 3, done: 3, total, failed: [{ title: "Song", error: "This text is not open, so it is never sent." }], stopped: null });
  assert.equal(lines[0], "Tagged and parsed 3 sentences (12 words) in 2 texts with udpipe english.udpipe.");
  assert.ok(lines.includes("«Song»: This text is not open, so it is never sent."));
  assert.ok(lines.some((l) => l.startsWith("1 sentence was left alone")));
  const stopped = projectTaggedLines({ texts: 5, done: 2, total: NO_TAGS, failed: [{ title: "A", error: "x" }, { title: "B", error: "x" }], stopped: "repeated" });
  assert.equal(stopped[0], "Stopped after 2 texts of 5: the last two failed the same way.");
  assert.equal(stopped.length, 3, "no claim that nothing needed tags when every text failed");
});

test("a run stops when two texts in a row fail the same way", () => {
  assert.equal(shouldStop(null, "x"), false);
  assert.equal(shouldStop("x", "x"), true);
  assert.equal(shouldStop("x", "y"), false);
  assert.equal(shouldStop("x", null), false, "a success resets it");
});
