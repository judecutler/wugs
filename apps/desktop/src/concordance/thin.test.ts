import assert from "node:assert/strict";
import { test } from "node:test";
import { thin, thinLine, type Hit } from "./logic.ts";

const hits: Hit[] = Array.from({ length: 1000 }, (_, i) => ({ textId: "t", textTitle: "T", tierId: "u", segmentId: `s${i}`, words: ["a", "b"], at: 0 }) as Hit);

test("thinning is a reproducible sample in corpus order", () => {
  const a = thin(hits, 100, "word|kani");
  assert.equal(a.length, 100);
  assert.deepEqual(a, thin(hits, 100, "word|kani"), "the same seed gives the same lines");
  assert.notDeepEqual(a.map((h) => h.segmentId), thin(hits, 100, "word|tela").map((h) => h.segmentId));
  const order = a.map((h) => Number(h.segmentId.slice(1)));
  assert.deepEqual(order, [...order].sort((x, y) => x - y));
  assert.equal(new Set(order).size, 100);
  // Spread over the whole list, not bunched at the start.
  assert.ok(order[99]! > 800 && order[0]! < 200);
  assert.equal(thin(hits.slice(0, 30), 50, "x").length, 30);
  assert.equal(thinLine(100, 2000, 5321), " Thinned to 100 at random of the first 2,000 found; the same query gives the same 100.");
  assert.equal(thinLine(50, 300, 300), " Thinned to 50 at random of 300 lines; the same query gives the same 50.");
});
