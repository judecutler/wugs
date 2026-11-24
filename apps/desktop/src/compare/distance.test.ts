import assert from "node:assert/strict";
import { test } from "node:test";
import { distanceLine, distances, MIN_COMPARED } from "./distance.ts";
import type { Row } from "./logic.ts";

const concept = (i: number) => ({ id: `c${i}`, number: i, gloss: `g${i}`, concepticonGloss: "", answers: [] });
const words = ["kana", "tupo", "mele", "sira", "wodu", "pake", "lumi", "neto", "gari", "boso", "fiku", "hata"];
const rows = (other: (w: string, i: number) => string): Row[] =>
  words.map((w, i) => ({ concept: concept(i), ours: [w], others: new Map([["Near", [{ form: other(w, i), similarity: null }]], ["Far", [{ form: words[(i + 5) % words.length]!, similarity: null }]]]) }));

test("a language whose words for the same things are alike comes first, with LDND well under 1", () => {
  const ds = distances(rows((w) => w.replace(/a$/u, "o")), ["Far", "Near"]);
  assert.equal(ds[0]!.language, "Near");
  assert.equal(ds[0]!.compared, words.length);
  assert.ok(ds[0]!.ldnd! < 0.4, JSON.stringify(ds));
  assert.ok(ds[1]!.ldnd! > 0.8, JSON.stringify(ds));
  assert.match(distanceLine(ds[0]!), /^Near: over 12 concepts, words for the same thing differ in \d+% of their sounds \(LDN\)\. LDND \d+%\.$/);
});

test("too few concepts gives no numbers", () => {
  const few = distances(rows((w) => w).slice(0, MIN_COMPARED - 1), ["Near"]);
  assert.match(distanceLine(few[0]!), /too few to measure/);
});
