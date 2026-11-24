import assert from "node:assert/strict";
import { test } from "node:test";
import { kindOf, measures, rhythm, rhythmLines, rhythmTsv, type Interval } from "./rhythm.ts";

const seg = (i: number, start: number, end: number, content: string, tier = "ph") => ({ id: `s${i}`, tier, startMs: start, endMs: end, content });

test("a phone track becomes vocalic and consonantal intervals within stretches", () => {
  assert.equal(kindOf("a"), "V");
  assert.equal(kindOf("tʰ"), "C");
  assert.equal(kindOf("sil"), null);
  assert.equal(kindOf("ai"), null);
  const r = rhythm(
    [
      seg(1, 0, 50, "s"),
      seg(2, 50, 80, "t"),
      seg(3, 80, 180, "a"),
      seg(4, 180, 240, "i"),
      seg(5, 240, 300, "k"),
      seg(6, 300, 600, "sil"),
      seg(7, 600, 650, "m"),
      seg(8, 700, 760, "o"),
      seg(9, 760, 800, "x", "other"),
      seg(10, 800, 800, "e"),
    ],
    "ph",
  );
  assert.equal(r.segments, 8);
  assert.equal(r.classed, 7);
  // s+t, a+i, k | sil | m | (gap of 50 ms) o.
  assert.deepEqual(r.stretches, [[{ kind: "C", ms: 80 }, { kind: "V", ms: 160 }, { kind: "C", ms: 60 }], [{ kind: "C", ms: 50 }], [{ kind: "V", ms: 60 }]]);
  assert.equal(rhythmTsv(r).split("\n")[1], "1\t1\tC\t80");
  assert.match(rhythmLines(r)[0]!, /Too few vowel and consonant intervals/);
  // A track of words is not a phone track: nothing to say.
  assert.deepEqual(rhythmLines(rhythm([seg(1, 0, 400, "kata"), seg(2, 400, 900, "mosi")], "ph")), []);
});

test("Ramus, Dellwo and Grabe and Low's measures", () => {
  const st: Interval[] = [];
  for (let i = 0; i < 10; i++) st.push({ kind: "C", ms: 80 }, { kind: "V", ms: i % 2 === 0 ? 100 : 50 });
  const m = measures({ segments: 20, classed: 20, stretches: [st] })!;
  assert.equal(m.v, 10);
  assert.ok(Math.abs(m.percentV - (100 * 750) / 1550) < 1e-9);
  assert.equal(m.deltaC, 0);
  assert.equal(m.rPviC, 0);
  // Every neighbouring pair is 100 against 50: |50| / 75.
  assert.ok(Math.abs(m.nPviV! - 200 / 3) < 1e-9);
  // Sample SD of five 100s and five 50s, over their mean 75.
  const sd = Math.sqrt((10 * 25 ** 2) / 9);
  assert.ok(Math.abs(m.deltaV - sd) < 1e-9 && Math.abs(m.varcoV - (100 * sd) / 75) < 1e-9);
  const lines = rhythmLines({ segments: 20, classed: 20, stretches: [st] });
  assert.match(lines[1]!, /^%V 48\.4, ΔV 26\.4 ms, ΔC 0\.0 ms/);
  assert.match(lines[2]!, /nPVI of the vowels 66\.7, rPVI of the consonants 0\.0 ms/);
  assert.equal(measures({ segments: 2, classed: 2, stretches: [[{ kind: "V", ms: 1 }, { kind: "C", ms: 1 }]] }), null);
});
