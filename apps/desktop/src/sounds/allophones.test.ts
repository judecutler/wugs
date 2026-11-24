import assert from "node:assert/strict";
import { test } from "node:test";
import { allophonesLine, featureDistance, possibleAllophones } from "./allophones.ts";

test("feature distance counts the features two sounds differ in", () => {
  assert.equal(featureDistance("p", "b"), 1);
  assert.equal(featureDistance("s", "ʃ"), 1);
  assert.ok((featureDistance("p", "a") ?? 0) > 5);
  assert.equal(featureDistance("p", "ǃ"), null);
});

test("similar sounds in complementary distribution are found, with a rule", () => {
  // t only at the end of words, d only between vowels: final devoicing.
  const words = [
    { word: "pada", count: 4 },
    { word: "kodu", count: 3 },
    { word: "pat", count: 4 },
    { word: "kut", count: 2 },
    { word: "pipi", count: 5 },
    { word: "papu", count: 3 },
  ];
  const found = possibleAllophones(words);
  const td = found.find((x) => [x.a, x.b].sort().join() === "d,t");
  assert.ok(td !== undefined, JSON.stringify(found));
  assert.equal(td.apart, 1);
  assert.equal(td.rule, "/d/ → [t] / _ a word edge");
  // p and b: b never occurs, so no pair; p is common but alone.
  assert.ok(found.every((x) => x.a !== "b" && x.b !== "b"));
  assert.match(allophonesLine(found), /never share an environment/);
  assert.match(allophonesLine([]), /^No two similar sounds/);
});

test("sounds that share an environment are not listed", () => {
  const words = [
    { word: "pata", count: 5 },
    { word: "pada", count: 5 },
  ];
  assert.ok(possibleAllophones(words).every((x) => [x.a, x.b].sort().join() !== "d,t"));
});
