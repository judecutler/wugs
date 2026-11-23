import { test } from "node:test";
import assert from "node:assert/strict";
import { clusterLine, clusters, clustersTsv } from "./clusters.ts";
import type { Hit } from "./logic.ts";

const hit = (text: string, words: string, at: number, len?: number): Hit => ({
  textId: text,
  textTitle: text,
  segmentId: `${text}-${words}`,
  tierId: "t",
  startMs: null,
  endMs: null,
  words: words.split(" "),
  at,
  len,
  breakdown: null,
  glosses: null,
});

const hits = [hit("a", "ni kani ŋai", 1), hit("a", "Kani ŋai toɾu", 0), hit("b", "ta kani haru", 1), hit("b", "kani", 0)];

test("clusters with the item first, last or anywhere", () => {
  const right = clusters(hits, 2, "right");
  assert.deepEqual(right.map((c) => [c.words.join(" "), c.count, c.texts]), [["kani ŋai", 2, 1], ["kani haru", 1, 1]]);
  assert.equal(right[0]!.at, 0);
  const left = clusters(hits, 2, "left");
  assert.deepEqual(left.map((c) => [c.words.join(" "), c.count]), [["ni kani", 1], ["ta kani", 1]]);
  const any = clusters(hits, 2, "any");
  assert.equal(any.reduce((n, c) => n + c.count, 0), 5, "a line counts once per place the cluster fits");
  assert.deepEqual(clusters(hits, 3, "any").map((c) => c.words.join(" ")), ["Kani ŋai toɾu", "ni kani ŋai", "ta kani haru"]);
});

test("a phrase longer than the cluster has none", () => {
  assert.deepEqual(clusters([hit("a", "ni kani ŋai", 0, 3)], 2, "any"), []);
  assert.equal(clusters([hit("a", "ni kani ŋai", 0, 2)], 3, "right")[0]!.len, 2);
});

test("the line and the table", () => {
  assert.equal(clusterLine(clusters(hits, 2, "right"), 4, 4, 2), "2 different clusters of 2 words.");
  assert.equal(clusterLine([], 4, 10, 4), "No line has 4 words around it. Counted over the first 4 of 10 lines; narrow the search to count them all.");
  assert.equal(clustersTsv(clusters(hits, 2, "right")), "cluster\tlines\ttexts\nkani ŋai\t2\t1\nkani haru\t1\t1");
});
