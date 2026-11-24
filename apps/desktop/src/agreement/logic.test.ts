import assert from "node:assert/strict";
import { test } from "node:test";
import {
  agreement,
  agreementLines,
  agreementTsv,
  cohensKappa,
  defaultTracks,
  differs,
  editDistance,
  kappaWords,
  units,
  type Seg,
} from "./logic.ts";

const seg = (
  id: string,
  tier: string,
  start: number | null,
  end: number | null,
  content: string,
): Seg => ({ id, tier, startMs: start, endMs: end, content });

test("units are sounds, with a space between words", () => {
  assert.deepEqual(units("ka  baː"), ["k", "a", " ", "b", "aː"]);
  assert.equal(units("").length, 0);
});

test("edit distance counts insertions, deletions and changes", () => {
  assert.equal(editDistance(["a", "b", "c"], ["a", "b", "c"]), 0);
  assert.equal(editDistance(["a", "b", "c"], ["a", "c"]), 1);
  assert.equal(editDistance(["k", "a", "t"], ["g", "a", "t", "a"]), 2);
  assert.equal(editDistance([], ["x", "y"]), 2);
});

test("Cohen's kappa matches a worked example", () => {
  // 50 items: both yes 20, A yes B no 5, A no B yes 10, both no 15.
  const items: [string, string][] = [
    ...Array<[string, string]>(20).fill(["y", "y"]),
    ...Array<[string, string]>(5).fill(["y", "n"]),
    ...Array<[string, string]>(10).fill(["n", "y"]),
    ...Array<[string, string]>(15).fill(["n", "n"]),
  ];
  // po = 0.7, pe = 0.5*0.6 + 0.5*0.4 = 0.5, kappa = 0.4
  assert.ok(Math.abs(cohensKappa(items)! - 0.4) < 1e-9);
  assert.equal(cohensKappa([]), null);
  assert.equal(
    cohensKappa([
      ["a", "a"],
      ["a", "a"],
    ]),
    null,
  );
  assert.equal(kappaWords(0.4), "fair");
  assert.equal(kappaWords(0.85), "almost perfect");
});

test("segments pair by overlap, each once, the rest unmatched", () => {
  const segs = [
    seg("a1", "A", 0, 1000, "kaba"),
    seg("a2", "A", 1000, 2000, "tula"),
    seg("a3", "A", 5000, 6000, "lone"),
    seg("a4", "A", null, null, "untimed"),
    seg("b1", "B", 20, 990, "kaba"),
    seg("b2", "B", 1100, 2050, "tulla"),
    seg("b3", "B", 1500, 1600, "tiny"), // overlaps a2 by 10%: not a pair
    seg("x", "C", 0, 1000, "other track"),
  ];
  const r = agreement(segs, "A", "B");
  assert.equal(r.pairs, 2);
  assert.equal(r.onlyA, 1);
  assert.equal(r.onlyB, 1);
  assert.equal(r.identical, 1);
  const pairs = r.rows.filter((x) => x.kind === "pair");
  assert.deepEqual(
    pairs.map((p) => (p.kind === "pair" ? [p.pair.a.id, p.pair.b.id] : [])),
    [
      ["a1", "b1"],
      ["a2", "b2"],
    ],
  );
  // kaba/kaba 0 of 4, tula/tulla 1 of 4
  assert.equal(r.errorRate, 1 / 8);
  assert.deepEqual(r.startDiff, { mean: 60, median: 60 });
  assert.deepEqual(r.endDiff, { mean: 30, median: 30 });
  assert.deepEqual(
    r.rows.map((x) => (x.kind === "pair" ? x.pair.a.id : x.seg.id)),
    ["a1", "a2", "b3", "a3"],
  );
  assert.equal(r.rows.filter(differs).length, 3);
  // 4 items: kaba=kaba, tula/tulla, tiny/∅... labels are categories here.
  assert.notEqual(r.kappa, null);
});

test("the best overlap wins when a segment could pair twice", () => {
  const segs = [
    seg("a1", "A", 0, 1000, "x"),
    seg("b1", "B", 0, 700, "x"),
    seg("b2", "B", 50, 1000, "x"),
  ];
  const r = agreement(segs, "A", "B");
  assert.equal(r.pairs, 1);
  const p = r.rows.find((x) => x.kind === "pair");
  assert.equal(p?.kind === "pair" ? p.pair.b.id : "", "b2");
});

test("a long segment far back still pairs with a later one", () => {
  const segs = [
    seg("a0", "A", 0, 100, "p"),
    seg("a1", "A", 9000, 10_000, "q"),
    seg("b0", "B", 8800, 10_000, "q"),
    seg("b1", "B", 10_500, 10_600, "r"),
  ];
  assert.equal(agreement(segs, "A", "B").pairs, 1);
});

test("many different labels give no kappa, and the lines say why", () => {
  const segs: Seg[] = [];
  for (let i = 0; i < 45; i++) {
    segs.push(
      seg(`a${i}`, "A", i * 1000, i * 1000 + 900, `word${i}`),
      seg(`b${i}`, "B", i * 1000, i * 1000 + 900, `word${i}`),
    );
  }
  const r = agreement(segs, "A", "B");
  assert.equal(r.kappa, null);
  const lines = agreementLines(r, "Ana", "Ben");
  assert.match(lines.join("\n"), /45 pairs/);
  assert.match(lines.join("\n"), /identical in 45 of 45 pairs \(100%\)/);
  assert.match(lines.join("\n"), /No kappa: 45 different labels/);
  assert.match(lines.join("\n"), /apart by 0 ms/);
  assert.deepEqual(agreementLines(agreement([], "A", "B"), "Ana", "Ben"), [
    "Neither «Ana» nor «Ben» has timed segments.",
  ]);
});

test("the table has a row per pair or unmatched segment", () => {
  const r = agreement(
    [
      seg("a", "A", 0, 1000, "ka\tba"),
      seg("b", "B", 0, 1000, "kaba"),
      seg("c", "B", 3000, 4000, "lone"),
    ],
    "A",
    "B",
  );
  const tsv = agreementTsv(r, "Ana", "Ben").split("\n");
  assert.equal(
    tsv[0],
    "Ana start\tAna end\tAna\tBen start\tBen end\tBen\toverlap\tidentical\tsounds apart",
  );
  assert.equal(tsv[1], "0.000\t1.000\tka ba\t0.000\t1.000\tkaba\t1.00\tno\t1");
  assert.equal(tsv[2], "\t\t\t3.000\t4.000\tlone\t\t\t");
});

test("the default tracks are the two fullest, in their order", () => {
  const tiers = [
    { id: "t1" },
    { id: "t2" },
    { id: "t3" },
    { id: "tr", role: "translation" },
  ];
  const segs = [
    seg("1", "t1", 0, 1, "a"),
    seg("2", "t3", 0, 1, "a"),
    seg("3", "t3", 2, 3, "b"),
    seg("4", "t2", 0, 1, "a"),
    seg("5", "t2", 2, 3, "a"),
    ...["x", "y", "z"].map((c, i) => seg(c, "tr", i, i + 1, c)),
  ];
  assert.deepEqual(defaultTracks(tiers, segs), ["t2", "t3"]);
  assert.equal(defaultTracks(tiers, [segs[0]!]), null);
});
