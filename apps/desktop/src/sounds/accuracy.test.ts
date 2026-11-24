import { test } from "node:test";
import assert from "node:assert/strict";
import { accuracy, accuracyNote, accuracyTsv, score, scoreCells, wordPairs } from "./accuracy.ts";

test("words pair up when the counts agree", () => {
  assert.deepEqual(wordPairs("bɔl dɔɡi .", "bɑ dɔdi"), [["bɔl", "bɑ"], ["dɔɡi", "dɔdi"]]);
  assert.deepEqual(wordPairs("bɔl dɔɡi", "bɑdɔdi"), [["bɔldɔɡi", "bɑdɔdi"]]);
  assert.deepEqual(wordPairs("", "bɑ"), []);
});

test("consonants and vowels right, deletions wrong, additions not counted", () => {
  const changes = new Map<string, number>();
  const s = score("bɔl dɔɡi", "bɑ dɔdi", undefined, changes);
  // Consonants b l d ɡ: b and d right, l left out, ɡ said as d.
  assert.deepEqual([s.consonants, s.consonantsRight, s.vowels, s.vowelsRight, s.words, s.wordsRight], [4, 2, 3, 2, 2, 0]);
  assert.deepEqual([...changes].sort(), [["l→∅", 1], ["ɔ→ɑ", 1], ["ɡ→d", 1]]);
  assert.deepEqual(scoreCells(s), ["50.0%", "66.7%", "57.1%", "0.0%"]);
  const plus = score("bɔl", "bɔlə");
  assert.deepEqual([plus.consonantsRight, plus.consonants, plus.vowels, plus.wordsRight], [2, 2, 1, 0], "an added schwa is not counted, but the word is not right");
});

test("a row per speaker and session, a total per speaker", () => {
  const data = {
    tracks: [["t1", "CHI"]] as [string, string][],
    omitted: 0,
    pairs: [
      { track: "t1", text: "a", title: "Bath", segment: "s1", target: "bɔl", actual: "bɑ" },
      { track: "t1", text: "b", title: "Park", segment: "s2", target: "bɔl", actual: "bɔl" },
    ],
  };
  const r = accuracy(data);
  assert.deepEqual(r.rows.map((x) => [x.title, x.score.consonantsRight, x.score.consonants]), [["Bath", 1, 2], ["Park", 2, 2]]);
  assert.deepEqual([r.totals[0]!.score.consonantsRight, r.totals[0]!.score.consonants], [3, 4]);
  assert.deepEqual(r.changes.get("t1"), [["l→∅", 1], ["ɔ→ɑ", 1]]);
  assert.match(accuracyNote(r.totals), /^Percent consonants correct \(PCC, Shriberg and Kwiatkowski 1982\) over every session: CHI 75\.0%\./u);
  assert.equal(accuracyTsv(r.rows).split("\n")[1], "CHI\tBath\t1\t50.0%\t0.0%\t33.3%\t0.0%");
});
