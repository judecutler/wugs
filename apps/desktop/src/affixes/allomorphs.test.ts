import assert from "node:assert/strict";
import { test } from "node:test";
import { allomorphSets, allomorphsTsv, neighbours, setNote, soundsLine } from "./allomorphs.ts";
import { marked, type Affix } from "./logic.ts";

const affix = (surface: string, gloss: string, slot: number, stems: [string, number][], kind = slot < 0 ? "prefix" : "suffix"): Affix => ({
  surface,
  kind,
  gloss,
  uses: stems.reduce((n, s) => n + s[1], 0),
  roots: stems.length,
  positions: [[slot, 1]],
  slot,
  categories: [],
  example: { word: "", textId: "", tierId: "", segmentId: "" },
  stems,
});

test("a suffix's neighbours are the stem's last sound and last vowel", () => {
  const n = neighbours(affix("ler", "PL", 1, [["ev", 3], ["kedi", 2], ["göz", 1]]));
  assert.deepEqual(n.next, [{ sound: "v", count: 3 }, { sound: "i", count: 2 }, { sound: "z", count: 1 }]);
  assert.deepEqual(n.vowel, [{ sound: "e", count: 3 }, { sound: "i", count: 2 }, { sound: "ö", count: 1 }]);
});

test("a prefix's neighbours are the stem's first sound and first vowel", () => {
  const n = neighbours(affix("an", "3SG", -1, [["ika", 2], ["tolu", 1]]));
  assert.deepEqual(n.next.map((c) => c.sound), ["i", "t"]);
  assert.deepEqual(n.vowel.map((c) => c.sound), ["i", "o"]);
});

test("forms of one gloss on one side make a set, and harmony is found", () => {
  const sets = allomorphSets([
    affix("lar", "PL", 1, [["kɯtɑp", 4], ["kuzu", 3], ["okol", 2]]),
    affix("ler", "PL", 1, [["ev", 5], ["kedi", 2], ["gyl", 1]]),
    affix("di", "PST", 1, [["gel", 2]]),
    affix("ni", "PL", -1, [["kitap", 1]]),
  ]);
  assert.equal(sets.length, 1);
  const pl = sets[0]!;
  assert.equal(pl.gloss, "PL");
  assert.equal(pl.uses, 17);
  assert.deepEqual(pl.members.map((m) => m.affix.surface), ["lar", "ler"]);
  assert.deepEqual(pl.tells, ["vowel"]);
  assert.match(setNote(pl), /last vowel keeps the forms apart: perhaps vowel harmony/);
  assert.match(soundsLine(pl.members[0]!.vowel), /^ɑ, u, o \(.*back.*\)$/);
  assert.match(soundsLine(pl.members[1]!.vowel), /^e, i, y \(.*front.*\)$/);
});

test("assimilation is found from the sound next to the affix", () => {
  const sets = allomorphSets([
    affix("s", "PL", 1, [["kat", 2], ["kap", 1]]),
    affix("z", "PL", 1, [["dɔg", 2], ["bɛd", 1], ["bi", 1]]),
  ]);
  assert.ok(sets[0]!.tells.includes("next"));
  assert.match(soundsLine(sets[0]!.members.find((m) => m.affix.surface === "s")!.next), /^t, p \(voiceless stops\)$/);
});

test("stems ending in the vowel itself point at the vowel", () => {
  const [s] = allomorphSets([affix("si", "PL", 1, [["kesi", 2], ["sele", 1]]), affix("su", "PL", 1, [["toɾu", 2], ["kanu", 1]])]);
  assert.deepEqual(s!.tells, ["next", "vowel"]);
  assert.match(setNote(s!), /stem's last vowel, right before it, keeps the forms apart: perhaps vowel harmony/);
});

test("free variants are said to be neither", () => {
  const [s] = allomorphSets([affix("ka", "Q", 2, [["mopa", 1], ["tela", 1]], "clitic"), affix("ga", "Q", 2, [["mopa", 1]], "clitic")]);
  assert.deepEqual(s!.tells, []);
  assert.match(setNote(s!), /free variants/);
  const [old] = allomorphSets([{ ...affix("ka", "Q", 1, []), stems: undefined }, affix("ga", "Q", 1, [["mopa", 1]])]);
  assert.match(setNote(old!), /no stems recorded/);
});

test("the table lists every form with its neighbours", () => {
  const sets = allomorphSets([affix("lar", "PL", 1, [["kuzu", 1]]), affix("ler", "PL", 1, [["kedi", 1]])]);
  const tsv = allomorphsTsv(sets, marked).split("\n");
  assert.equal(tsv[0], "gloss\tform\tuses\troots\tnext to\tnearest vowel");
  assert.equal(tsv[1], "PL\t-lar\t1\t1\tu\tu");
  assert.equal(soundsLine([]), "–");
});
