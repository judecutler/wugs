import { test } from "node:test";
import assert from "node:assert/strict";
import { affixLabel, guessedLine, signatureLabel, signatures, signaturesTsv } from "./signatures.ts";

const words = (list: string) => list.split(/\s+/).filter(Boolean).map((word) => ({ word, count: 2 }));

test("stems that share the same endings make one signature", () => {
  const g = signatures(words("walk walks walked walking talk talks talked talking jump jumps jumped jumping cat dog tree"), "suffix");
  const sig = g.signatures.find((s) => signatureLabel(s) === "∅.s.ed.ing");
  assert.ok(sig, g.signatures.map(signatureLabel).join(" "));
  assert.deepEqual(sig.stems.map((s) => s.stem).sort(), ["jump", "talk", "walk"]);
  assert.equal(sig.tokens, 24);
  assert.equal(g.signatures[0], sig);
  // Each word is read once: no wal.ks.ked.king beside it.
  assert.equal(g.signatures.length, 1);
  assert.deepEqual(g.affixes.slice(0, 3).map((a) => a.stems), [3, 3, 3]);
});

test("beginnings are found the same way from the other end", () => {
  const g = signatures(words("tidy untidy retidy fold unfold refold wrap unwrap rewrap"), "prefix");
  const sig = g.signatures[0];
  assert.equal(signatureLabel(sig), "∅.re.un");
  assert.equal(affixLabel("un", "prefix"), "un-");
  assert.equal(affixLabel("ed", "suffix"), "-ed");
  assert.equal(affixLabel("", "suffix"), "∅");
});

test("an ending met on fewer than three stems makes no signature, and letters only are read", () => {
  const g = signatures(words("walk walks talk talks 12 a-b"), "suffix");
  assert.equal(g.signatures.length, 0);
  assert.equal(g.read, 4);
  assert.match(guessedLine(g, "suffix"), /No endings are shared by 3 or more stems in 4 word forms/);
  assert.equal(guessedLine(signatures([], "suffix"), "suffix"), "No words to read yet: transcribe some sentences first.");
});

test("case is folded, marks stay with their letter, and the table lists every signature", () => {
  const g = signatures(words("Kanö kanöla kanöli Tepö tepöla tepöli Sirö siröla siröli"), "suffix");
  assert.equal(signatureLabel(g.signatures[0]), "∅.la.li");
  const tsv = signaturesTsv(g, "suffix").split("\n");
  assert.equal(tsv[0], "Endings\tStems\tWords\tStem list");
  assert.ok(tsv[1].startsWith("∅ -la -li\t3\t"));
  assert.match(guessedLine(g, "suffix"), /Met on most stems: -la \(3\), -li \(3\)/);
});
