import { test } from "node:test";
import assert from "node:assert/strict";
import { matchLine, pairUp, spokenInOrder } from "./match.ts";

const seg = (id: string, startMs: number | null, endMs: number | null) => ({ id, startMs, endMs, content: "" });

test("words pair with the timed segments in time order", () => {
  const segments = [seg("b", 900, 1400), seg("x", null, null), seg("a", 100, 600), seg("c", 1600, 2000)];
  assert.deepEqual(spokenInOrder(segments).map((s) => s.id), ["a", "b", "c"]);
  const pairs = pairUp([{ id: "w1", headword: "kani" }, { id: "w2", headword: "ŋai" }], segments);
  assert.deepEqual(pairs.map((p) => [p.word.headword, p.segment.id]), [["kani", "a"], ["ŋai", "b"]]);
});

test("the line says whether the counts line up", () => {
  assert.match(matchLine(3, 3), /^3 words and 3 segments: each word takes/);
  assert.match(matchLine(4, 3), /1 word is left without a segment/);
  assert.match(matchLine(2, 3), /1 segment is left over/);
  assert.match(matchLine(2, 0), /Auto-segment the recording by word/);
});
