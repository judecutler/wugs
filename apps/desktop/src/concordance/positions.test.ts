import { test } from "node:test";
import assert from "node:assert/strict";
import { positions, positionsTsv } from "./positions.ts";
import type { Hit } from "./logic.ts";

const hit = (words: string[], at: number, len = 1) => ({ textId: "t", textTitle: "T", segmentId: "s", tierId: "u", startMs: null, endMs: null, words, at, len, breakdown: null, glosses: null }) as Hit;

test("words are counted position by position around the whole match", () => {
  const hits = [hit(["The", "big", "dog", "ran", "home."], 2), hit(["a", "big", "dog", "ran"], 2), hit(["the", "hot", "dog", "stand"], 2), hit(["big", "red", "dog", "barked"], 1, 2)];
  const cols = positions(hits, 2, 3);
  assert.deepEqual(cols.map((c) => c.label), ["2 before", "1 before", "1 after", "2 after"]);
  assert.deepEqual(cols[1]!.rows, [{ word: "big", count: 3 }, { word: "hot", count: 1 }]);
  assert.deepEqual(cols[0]!.rows, [{ word: "the", count: 2 }, { word: "a", count: 1 }]);
  // After a two-word match, the word after its end.
  assert.deepEqual(cols[2]!.rows, [{ word: "ran", count: 2 }, { word: "barked", count: 1 }, { word: "stand", count: 1 }]);
  assert.deepEqual(cols[3]!.rows, [{ word: "home", count: 1 }]);
  assert.equal(cols[3]!.lines, 1);
  assert.equal(positionsTsv([cols[1]!, cols[3]!]), "rank\t1 before\tn\t2 after\tn\n1\tbig\t3\thome\t1\n2\thot\t1\t\t\n");
});
