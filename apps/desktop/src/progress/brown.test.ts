import { test } from "node:test";
import assert from "node:assert/strict";
import { brownNote, brownRows, brownTsv } from "./brown.ts";

const counts = (at: Record<number, number>) => Array.from({ length: 14 }, (_, i) => at[i] ?? 0);
const track = {
  tier: "t",
  name: "CHI",
  sessions: [
    { text: "a", title: "2;03", utterances: 40, counts: counts({ 0: 3, 3: 5 }) },
    { text: "b", title: "2;09", utterances: 60, counts: counts({ 0: 8, 3: 9, 7: 4 }) },
  ],
};

test("a row per morpheme, a column per session", () => {
  const rows = brownRows(track);
  assert.equal(rows.length, 14);
  assert.deepEqual(rows[0], { morpheme: "Present progressive -ing", counts: [3, 8], total: 11 });
  assert.deepEqual(rows[7]!.counts, [0, 4]);
  assert.equal(brownTsv(track).split("\n")[0], "morpheme\t2;03\t2;09\ttotal");
  assert.equal(brownTsv(track).split("\n")[8], "Articles\t0\t4\t4");
});

test("the note says what was used and what came last", () => {
  assert.match(brownNote(track), /^CHI uses 3 of Brown's 14 morphemes in 100 utterances with %mor\. Last to appear: Articles \(from «2;09»\)\./u);
});
