import assert from "node:assert/strict";
import { test } from "node:test";
import { exactMatches, filterConcepts, isDone, nextOnTrack, nextTodo, progressLabel, progressShare, soundLine, soundSegment, ownListLine, wordlistLines, coverageLine, nextEmptyDomain } from "./logic.ts";

const c = (id: string, answered: boolean, skipped = false) => ({ id, skipped, answers: answered ? [{}] : [] });
const list = [c("a", true), c("b", false), c("c", false, true), c("d", false)];

test("done means answered or not applicable", () => {
  assert.deepEqual(list.map(isDone), [true, false, true, false]);
  assert.deepEqual(filterConcepts(list, "todo").map((x) => x.id), ["b", "d"]);
  assert.deepEqual(filterConcepts(list, "done").map((x) => x.id), ["a", "c"]);
  assert.equal(filterConcepts(list, "all").length, 4);
});

test("the next concept to do follows list order and wraps", () => {
  assert.equal(nextTodo(list, null)?.id, "b");
  assert.equal(nextTodo(list, "b")?.id, "d");
  assert.equal(nextTodo(list, "d")?.id, "b");
  assert.equal(nextTodo([c("x", true)], "x"), null);
  assert.equal(nextTodo([], null), null);
});

test("progress reads naturally", () => {
  assert.equal(progressLabel({ total: 100, answered: 37, skipped: 0 }), "37 of 100 answered");
  assert.equal(progressLabel({ total: 100, answered: 37, skipped: 2 }), "37 of 100 answered · 2 not applicable");
  assert.equal(progressShare({ total: 100, answered: 37, skipped: 3 }), 0.4);
  assert.equal(progressShare({ total: 0, answered: 0, skipped: 0 }), 0);
});

test("only exact lemmas are offered, case and NFC folded", () => {
  const rows = [{ lemma: "Su" }, { lemma: "sular" }, { lemma: "su" }];
  assert.deepEqual(exactMatches(rows, " SU "), [{ lemma: "Su" }, { lemma: "su" }]);
  assert.deepEqual(exactMatches(rows, ""), []);
  assert.deepEqual(exactMatches([{ lemma: "köpek" }], "köpek").length, 1);
});

test("answers take their sound from the selected segment, then the next on its track", () => {
  const seg = (id: string, tier: string, startMs: number | null, content = "") => ({ id, tier, startMs, endMs: startMs === null ? null : startMs + 500, content });
  const segments = [seg("a", "words", 0, "ateş"), seg("b", "words", 1000), seg("t", "notes", 500), seg("c", "words", 2000), seg("u", "words", null)];
  const text = { id: "txt", segments };
  assert.equal(soundSegment({ textId: "txt", segmentId: "a" }, text)?.id, "a");
  assert.equal(soundSegment({ textId: "other", segmentId: "a" }, text), null, "another text's selection");
  assert.equal(soundSegment({ textId: "txt", segmentId: "u" }, text), null, "no times, nothing to hear");
  assert.equal(soundSegment(null, text), null);
  assert.equal(nextOnTrack(segments, segments[0]!)?.id, "b", "the notes track is not this track");
  assert.equal(nextOnTrack(segments, segments[3]!), null, "the last one");
  const clock = (ms: number) => `${ms / 1000}s`;
  assert.equal(soundLine(segments[0]!, clock), "The next answer takes its sound from 0s–0.5s «ateş».");
  assert.match(soundLine(null, clock), /^Select a segment on the Timeline/u);
});

test("the word list export is reported in words", () => {
  const r = { path: "/x/wugs-wordlist-2026-11-23", lists: 1, concepts: 100, answered: 40, forms: 41, skipped: 2, byProfile: false, notes: ["A note."] };
  const lines = wordlistLines(r);
  assert.equal(lines[0], "41 forms for 40 of 100 concepts written as a CLDF Wordlist, with edictor.tsv for EDICTOR and LingPy, in /x/wugs-wordlist-2026-11-23");
  assert.match(lines[1]!, /letter by letter/);
  assert.equal(lines[2], "2 concepts are marked not applicable and have no form.");
  assert.equal(lines[3], "A note.");
  assert.match(wordlistLines({ ...r, byProfile: true, skipped: 1, notes: [] })[2]!, /^1 concept is marked not applicable and has no form\.$/);
});

test("loading a word list of one's own is reported in words", () => {
  assert.equal(ownListLine({ summary: { name: "Body parts", total: 2 }, created: 2, already: 0, withConcepticon: 2 }), "«Body parts»: 2 concepts loaded. 2 concepts are linked to Concepticon.");
  assert.match(ownListLine({ summary: { name: "B", total: 3 }, created: 1, already: 2, withConcepticon: 0 }), /^«B»: 1 new concept added, 2 already there\. None names/);
});

test("domain coverage and the next domain still empty", () => {
  const all = [
    { id: "1", depth: 0, words: 2 },
    { id: "1.1", depth: 1, words: 0 },
    { id: "1.2", depth: 1, words: 1 },
    { id: "2", depth: 0 },
  ];
  assert.equal(coverageLine(all), "3 words in 2 of 4 domains (50%).");
  assert.equal(coverageLine(all.map((d) => ({ ...d, words: 0 }))), "No words collected yet in any of the 4 domains.");
  assert.equal(nextEmptyDomain(all, null)?.id, "1.1");
  assert.equal(nextEmptyDomain(all, "1.1")?.id, "2");
  assert.equal(nextEmptyDomain(all, "2")?.id, "1.1", "wraps round");
  assert.equal(nextEmptyDomain(all.map((d) => ({ ...d, words: 1 })), "1"), null);
});
