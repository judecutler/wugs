import assert from "node:assert/strict";
import { test } from "node:test";
import { asTsv, keywordQuery, keywordRows, significant, summary, type KeywordRow } from "./logic.ts";

const row = (item: string, ll: number, lr: number): KeywordRow => ({ item, focus: 3, reference: 1, logLikelihood: ll, logRatio: lr, percentDiff: 50, simpleMaths: 2 ** lr });
const rows = [row("fox", 12, 4), row("the", 2, 0.5), row("kesi", -9, -3), row("ŋai", -1, -5)];

test("splits by side and sorts", () => {
  assert.deepEqual(keywordRows(rows, "more", "ll").map((r) => r.item), ["fox", "the"]);
  assert.deepEqual(keywordRows(rows, "less", "ll").map((r) => r.item), ["kesi", "ŋai"], "most negative first");
  assert.deepEqual(keywordRows(rows, "less", "ratio").map((r) => r.item), ["ŋai", "kesi"]);
  assert.equal(significant(rows[0]!, 3.84), true);
  assert.equal(significant(rows[0]!, 16.45), false, "Bonferroni over many words");
  assert.deepEqual(keywordRows(rows, "more", "sm").map((r) => r.item), ["fox", "the"]);
});

test("summary and table", () => {
  assert.match(summary("Fox", { rows, focusTokens: 8, referenceTokens: 0, types: 4, omitted: 0, critical: 9 }), /nothing to compare/);
  assert.match(summary("Fox", { rows, focusTokens: 8, referenceTokens: 1200, types: 4, omitted: 0, critical: 9 }), /«Fox» \(8 words\) against .* \(1,200 words\)/);
  assert.match(summary("Texts whose genre is «fable»", { rows, focusTokens: 8, referenceTokens: 1200, types: 4, omitted: 0, critical: 9 }, true), /^Texts whose genre is «fable» \(8 words\) against/);
  assert.equal(summary("Texts whose genre is «fable»", { rows, focusTokens: 0, referenceTokens: 1200, types: 0, omitted: 0, critical: 9 }, true), "Texts whose genre is «fable» have no words yet.");
  assert.match(summary("Fox", { rows, focusTokens: 8, referenceTokens: 1200, types: 4, omitted: 12, critical: 9.14 }), /9\.14 or more\)\. The 12 weakest are left out\./);
  // Against one other text, or the texts of a detail.
  assert.match(summary("Fox", { rows, focusTokens: 8, referenceTokens: 3, types: 4, omitted: 0, critical: 9 }, false, "«Hen»"), /^«Fox» \(8 words\) against «Hen» \(3 words\)/);
  assert.equal(summary("Fox", { rows, focusTokens: 8, referenceTokens: 0, types: 4, omitted: 0, critical: 9 }, false, "«Fox»"), "There is nothing to compare with: «Fox» has no words outside «Fox».");
  assert.equal(summary("Texts whose genre is «fable»", { rows, focusTokens: 8, referenceTokens: 0, types: 4, omitted: 0, critical: 9 }, true, "texts whose genre is «tale»"), "There is nothing to compare with: texts whose genre is «tale» have no words outside texts whose genre is «fable».");
  assert.equal(asTsv([row("fox", 12, 4)]).split("\n")[1], "fox\t3\t1\t12.00\t4.00\t50.0\t16.000");
});

test("lemma and tag rows open as CQP queries, sequences token by token", () => {
  assert.equal(keywordQuery("tag", "VERB", 1, null), '[upos="VERB"]');
  assert.equal(keywordQuery("tag", "ADJ NOUN", 2, null), '[upos="ADJ"] [upos="NOUN"]');
  assert.equal(keywordQuery("lemma", "ice cream", 1, null), '[lemma="ice cream"]', "a single lemma keeps its space");
  assert.equal(keywordQuery("lemma", "run", 1, { key: "genre", value: "fable" }), '[lemma="run"] :: match.text_genre="fable"');
});

test("the summary counts what was compared", () => {
  const k = { rows: [], focusTokens: 6, referenceTokens: 4, types: 3, omitted: 0, critical: 3.84 };
  assert.match(summary("Story", k, false, null, "tag"), /6 tagged words\) against the project's other texts \(4 tagged words\)/u);
  assert.match(summary("Story", { ...k, focusTokens: 0 }, false, null, "lemma"), /no lemmatised words yet\. Tag and parse/u);
  assert.match(asTsv([], "tag"), /^tag\tin text/u);
});
