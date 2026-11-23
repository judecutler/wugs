import { test } from "node:test";
import assert from "node:assert/strict";
import { asTsv, filterRows, lexiconCell, lexiconNote, notInLexicon, perThousand, ranked, sortRows, summary } from "./logic.ts";

const rows = [
  { item: "kesi", count: 5, texts: 1 },
  { item: "ŋai", count: 3, texts: 3 },
  { item: "Ata", count: 3, texts: 2 },
];

test("sorts by count (as given), alphabetically or by spread", () => {
  assert.deepEqual(sortRows(rows, "count").map((r) => r.item), ["kesi", "ŋai", "Ata"]);
  assert.deepEqual(sortRows(rows, "alpha").map((r) => r.item), ["Ata", "kesi", "ŋai"]);
  assert.deepEqual(sortRows(rows, "texts").map((r) => r.item), ["ŋai", "Ata", "kesi"]);
});

test("per thousand keeps one decimal only for small rates", () => {
  assert.equal(perThousand(3, 1000), "3.0");
  assert.equal(perThousand(25, 1000), "25");
  assert.equal(perThousand(1, 0), "0");
});

test("filter ignores case, not diacritics", () => {
  assert.deepEqual(filterRows(rows, "AT").map((r) => r.item), ["Ata"]);
  assert.deepEqual(filterRows(rows, "na").map((r) => r.item), []);
  assert.equal(filterRows(rows, " ").length, 3);
});

test("summary and table", () => {
  const noun = { one: "word", many: "words" };
  assert.equal(summary({ rows: [], tokens: 0, types: 0 }, noun), "No words yet.");
  assert.equal(summary({ rows, tokens: 11, types: 3 }, noun), "11 words, 3 different.");
  assert.match(summary({ rows, tokens: 1200, types: 9000 }, noun), /The 3 most frequent are listed/);
  assert.equal(asTsv(ranked(rows).slice(0, 1), 10).split("\n")[1], "1\tkesi\t5\t500\t1");
});

test("a row keeps its frequency rank through sorting and filtering", () => {
  const r = sortRows(filterRows(ranked(rows), "a"), "alpha");
  assert.deepEqual(r.map((x) => [x.item, x.rank]), [["Ata", 3], ["ŋai", 2]]);
  assert.equal(asTsv(r, 11).split("\n")[1]!.split("\t")[0], "3");
});

test("lemma and tag rows open the CQP query that finds their lines", async () => {
  const { cqpFor, cqpLiteral, unlabelledNote } = await import("./logic.ts");
  assert.equal(cqpLiteral("a.b"), "a\\.b");
  assert.equal(cqpLiteral('x"y'), 'x\\"y');
  assert.equal(cqpLiteral("c++ (1|2) ~-"), "c\\+\\+ \\(1\\|2\\) \\~\\-");
  assert.equal(cqpFor("lemma", "go", null), '[lemma="go"]');
  assert.equal(cqpFor("tag", "NOUN", { key: "genre", value: "news" }), '[upos="NOUN"] :: match.text_genre="news"');
  assert.equal(unlabelledNote({ rows: [], tokens: 3, types: 1, unlabelled: 0 }, "lemma"), "");
  assert.match(unlabelledNote({ rows: [], tokens: 3, types: 1, unlabelled: 2 }, "tag"), /^ 2 words have no tag yet: /);
});

test("a word list says what the lexicon lacks", () => {
  const words = ranked([
    { item: "kesi", count: 5, texts: 1, lexicon: "entry" as const },
    { item: "tolo", count: 3, texts: 1, lexicon: "none" as const },
    { item: "zibu", count: 2, texts: 2, lexicon: "analysed" as const },
  ]);
  assert.deepEqual(notInLexicon(words).map((r) => r.item), ["tolo", "zibu"]);
  assert.equal(lexiconNote(words), " 2 of these 3 words are not in the lexicon yet (used 5 times).");
  assert.equal(lexiconNote(words.slice(0, 1)), " Every word listed is in the lexicon.");
  assert.equal(lexiconNote(words.slice(1, 2)), " This word is not in the lexicon yet (used 3 times).");
  assert.equal(lexiconNote([{ item: "x", count: 1, texts: 1 }]), "", "morph lists carry no lexicon column");
  assert.equal(lexiconCell("analysed"), "Glossed only");
  assert.equal(asTsv(words, 10).split("\n")[0], "rank\titem\tcount\tper 1000 words\ttexts\tin lexicon");
  assert.equal(asTsv(words, 10).split("\n")[2], "2\ttolo\t3\t300\t1\tNo");
});
