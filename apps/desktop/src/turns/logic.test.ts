import assert from "node:assert/strict";
import { test } from "node:test";
import { byPair, histogram, summarize, turns, turnsLines, turnsTsv, type Seg } from "./logic.ts";

const seg = (id: string, tier: string, start: number, end: number): Seg => ({ id, tier, startMs: start, endMs: end, content: "x" });

const talk = [
  seg("a1", "A", 0, 1000),
  seg("b1", "B", 1200, 2000), // gap +200
  seg("a-mm", "A", 1500, 1700), // inside B's turn: backchannel
  seg("a2", "A", 1900, 3000), // overlap −100
  seg("a3", "A", 3100, 3500), // A goes on
  seg("b2", "B", 4500, 5000), // gap +1000
  seg("c1", "C", 4000, 4200), // starts before b2: sorted first, gap +500 after a3
  seg("x", "other", 0, 99999),
];

test("changes of speaker give offsets; backchannels and continuations do not", () => {
  const t = turns(talk, ["A", "B", "C"]);
  assert.equal(t.segments, 7);
  assert.deepEqual(t.transitions.map((x) => [x.from, x.to, x.offset]), [["A", "B", 200], ["B", "A", -100], ["A", "C", 500], ["C", "B", 300]]);
  assert.equal(t.inside, 1);
  assert.equal(t.continued, 1);
});

test("the summary counts gaps, overlaps and near-zero offsets", () => {
  const s = summarize(turns(talk, ["A", "B", "C"]).transitions)!;
  assert.deepEqual(s, { n: 4, mean: 225, median: 250, gaps: 2, overlaps: 0, near: 2 });
  assert.equal(summarize([]), null);
});

test("the histogram has 100 ms bins from −2 s to 2 s, clipping the ends", () => {
  const t = turns([seg("a", "A", 0, 5000), seg("b", "B", 100, 6000), seg("c", "A", 9000, 9500), seg("d", "B", 9550, 9900)], ["A", "B"]);
  const h = histogram(t.transitions);
  assert.equal(h.length, 40);
  assert.equal(h[0]!.from, -2000);
  assert.equal(h[0]!.count, 1); // −4900 clipped
  assert.equal(h.find((b) => b.from === 0)!.count, 1); // +50
  assert.equal(h.at(-1)!.count, 1); // +3000 clipped
});

test("each direction gets its own median", () => {
  const pairs = byPair(turns(talk, ["A", "B", "C"]).transitions);
  assert.deepEqual(pairs.map((p) => [p.from, p.to, p.summary.n, p.summary.median]), [["A", "B", 1, 200], ["A", "C", 1, 500], ["B", "A", 1, -100], ["C", "B", 1, 300]]);
});

test("lines in words, and a table", () => {
  const t = turns(talk, ["A", "B", "C"]);
  const lines = turnsLines(t, summarize(t.transitions));
  assert.equal(lines[0], "4 changes of speaker. Median offset +250 ms, mean +225 ms.");
  assert.equal(lines[1], "50% start after a gap of more than 200 ms, 0% overlap the previous turn by more than 200 ms, 50% come within 200 ms of its end.");
  assert.equal(lines[2], "1 segment falls wholly inside another speaker's turn (backchannels, not counted); 1 segment continues the same speaker's turn.");
  assert.deepEqual(turnsLines(turns([], ["A"]), null), ["No timed segments on these tracks."]);
  assert.match(turnsLines(turns([seg("a", "A", 0, 1)], ["A"]), null)[0]!, /No change of speaker/);
  const tsv = turnsTsv(t.transitions, (x) => `Speaker ${x}`).split("\n");
  assert.equal(tsv[0], "from\tto\tprevious end\tnext start\toffset ms");
  assert.equal(tsv[2], "Speaker B\tSpeaker A\t2.000\t1.900\t-100");
});
