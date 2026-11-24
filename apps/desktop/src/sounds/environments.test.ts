import assert from "node:assert/strict";
import { test } from "node:test";
import { distributionNote, envLabel, environments, environmentsTsv } from "./environments.ts";

// s before a, o, u and at the end; ʃ only before i and e: complementary.
const words = [
  { word: "sasa", count: 3 },
  { word: "ʃima", count: 2 },
  { word: "muʃe", count: 1 },
  { word: "kus", count: 4 },
  { word: "sopa", count: 1 },
];

test("environments are counted on each side and whole", () => {
  const e = environments(words, "s", "ʃ");
  assert.equal(e.countA, 3 * 2 + 4 + 1);
  assert.equal(e.countB, 3);
  assert.deepEqual(e.whole.map(envLabel), ["u_#", "#_a", "a_a", "#_i", "u_e", "#_o"]);
  assert.deepEqual(e.before.map((r) => [r.neighbour, r.a, r.b]), [["a", 6, 0], ["#", 4, 0], ["i", 0, 2], ["e", 0, 1], ["o", 1, 0]]);
  assert.equal(
    distributionNote(e),
    "«s» and «ʃ» never occur in the same environment in these words, so they may be allophones of one sound in complementary distribution. «s» only before a, #, o (vowels or a word edge), «ʃ» only before i, e (front vowels).",
  );
  assert.equal(environmentsTsv(e).split("\n")[1], "u_#\t4\t0\tkus\t");
});

test("a shared environment is named, and missing sounds said", () => {
  const e = environments([...words, { word: "aʃa", count: 1 }], "s", "ʃ");
  assert.match(distributionNote(e), /^«s» and «ʃ» share 1 environment \(a_a\): there they contrast/);
  assert.equal(distributionNote(environments(words, "s", "z")), "«z» is not in these words.");
  assert.equal(distributionNote(environments(words, "s", "s")), "Choose two different sounds.");
});
