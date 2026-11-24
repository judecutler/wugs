import assert from "node:assert/strict";
import { test } from "node:test";
import { network } from "./network.ts";
import type { CollocateRow } from "./logic.ts";

const row = (item: string, logDice: number, together = 3, ll = 5): CollocateRow => ({ item, together, left: 1, right: 2, freq: 9, logLikelihood: ll, mutualInformation: 1, logDice, tTest: 0, tScore: 1 } as CollocateRow);

test("one word: its strongest collocates around it, stronger nearer", () => {
  const n = network([{ word: "dog", rows: [row("bark", 12), row("the", 8), row("rare", 13, 1, -2), row("Dog", 11)] }]);
  assert.deepEqual(n.nodes.map((x) => x.label), ["dog", "bark", "the"], "rarer than chance and the word itself left out");
  const dist = (label: string) => Math.hypot(n.nodes.find((x) => x.label === label)!.x, n.nodes.find((x) => x.label === label)!.y);
  assert.ok(dist("bark") < dist("the"));
  assert.equal(n.edges.length, 2);
});

test("two words: shared collocates between them, each word's own outside", () => {
  const n = network([
    { word: "strong", rows: [row("tea", 12), row("wind", 10), row("argument", 9), row("powerful", 8)] },
    { word: "powerful", rows: [row("argument", 11), row("engine", 10)] },
  ]);
  const at = (label: string) => n.nodes.find((x) => x.label === label)!;
  assert.equal(at("argument").kind, "shared");
  assert.ok(Math.abs(at("argument").x) < 0.5);
  assert.ok(at("tea").x < -0.5 && at("engine").x > 0.5);
  assert.equal(n.edges.filter((e) => e.to === at("argument").id).length, 2);
  assert.equal(n.nodes.filter((x) => x.label === "powerful").length, 1, "the second word is a centre, not also a collocate");
});
