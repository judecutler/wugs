import assert from "node:assert/strict";
import { test } from "node:test";
import { categoriesOf, domainMatches, filterDomains, glossLine, headword, nextHomograph, parseHomograph, sameLemmaNote, usageLabel, relationLabel, relatedName, LIST_SHOWS, showLabel, shows, tableImportLines, loanSources, loansLine } from "./logic.ts";

test("a numbered homograph shows a subscript", () => {
  assert.equal(headword("yüz", null), "yüz");
  assert.equal(headword("yüz", 2), "yüz₂");
  assert.equal(headword("yüz", 12), "yüz₁₂");
});

test("glosses number only when there are several", () => {
  assert.equal(glossLine([]), "");
  assert.equal(glossLine(["moon"]), "moon");
  assert.equal(glossLine(["moon", " ", "month"]), "1. moon  2. month");
});

const domains = [
  { code: "1", label: "Universe, creation" },
  { code: "1.1", label: "Sky" },
  { code: "1.1.1", label: "Sun" },
  { code: "1.1.1.1", label: "Moon" },
  { code: "1.10", label: "Something" },
  { code: "2.1", label: "Body" },
  { code: "5.2.3.1", label: "Café, meal" },
];

test("a number searches the outline, including everything under it", () => {
  assert.deepEqual(filterDomains(domains, "1.1").map((d) => d.code), ["1.1", "1.1.1", "1.1.1.1"]);
  assert.deepEqual(filterDomains(domains, "1.1.").map((d) => d.code), ["1.1.1", "1.1.1.1"]);
  assert.equal(domainMatches(domains[4]!, "1.1"), false, "1.10 is not under 1.1");
});

test("words search the label, folded, every word must appear", () => {
  assert.deepEqual(filterDomains(domains, "MOON").map((d) => d.code), ["1.1.1.1"]);
  assert.deepEqual(filterDomains(domains, "cafe").map((d) => d.code), ["5.2.3.1"]);
  assert.deepEqual(filterDomains(domains, "universe creation").map((d) => d.code), ["1"]);
  assert.deepEqual(filterDomains(domains, "universe sky"), []);
  assert.equal(filterDomains(domains, "", 3).length, 3, "the limit holds");
});

test("homograph numbers parse strictly", () => {
  assert.deepEqual(parseHomograph(" "), { ok: true, value: null });
  assert.deepEqual(parseHomograph("2"), { ok: true, value: 2 });
  for (const bad of ["0", "-1", "1.5", "x", "1000"]) assert.deepEqual(parseHomograph(bad), { ok: false }, bad);
  assert.equal(nextHomograph([null, 1, 3]), 2);
  assert.equal(nextHomograph([]), 1);
});

test("the same-lemma notice asks for numbers only while some are missing", () => {
  assert.equal(sameLemmaNote("dil", null, []), null);
  assert.match(sameLemmaNote("dil", null, [null])!, /Number them/);
  assert.match(sameLemmaNote("dil", 1, [2, null])!, /^2 other entries.*Number them/);
  assert.match(sameLemmaNote("dil", 1, [2])!, /told apart/);
});

test("usage reads naturally", () => {
  assert.equal(usageLabel(0), "not used in any text yet");
  assert.equal(usageLabel(1), "used in 1 word");
  assert.equal(usageLabel(4), "used in 4 words");
});

test("relations read in words from the entry they are shown under", () => {
  assert.equal(relationLabel("compare"), "See also");
  assert.equal(relationLabel("whole"), "Has part");
  assert.equal(relationLabel("calendar"), "Calendar", "a file's own type is shown as written");
  assert.equal(relatedName({ otherLemma: "tela", otherHomograph: 2, otherSense: null }), headword("tela", 2));
  assert.equal(relatedName({ otherLemma: "laki", otherHomograph: null, otherSense: 2 }), "laki (sense 2)");
});

test("the entry list can show only the entries with work left", () => {
  const rows = [
    { glosses: ["dog"], posOfSpeech: "n", used: true },
    { glosses: [""], posOfSpeech: "", used: false },
    { glosses: [], posOfSpeech: "v", used: true },
    { glosses: ["eat", " "], posOfSpeech: "v", used: false },
  ];
  assert.deepEqual(rows.map((r) => shows(r, "gloss")), [false, true, true, true]);
  assert.deepEqual(rows.map((r) => shows(r, "category")), [false, true, false, false]);
  assert.deepEqual(rows.map((r) => shows(r, "unused")), [false, true, false, true]);
  assert.ok(rows.every((r) => shows(r, "all")));
  assert.equal(showLabel(rows, LIST_SHOWS[1]!), "Needs a gloss (3)");
});

test("a lexicon table import is reported in words", () => {
  const r = { entries: 2, senses: 3, unchanged: 1, ambiguous: ["lari"], noLemma: 1, columns: ["Headword → lemma", "English → gloss"], ignored: ["Colour"], domains: 0, domainsUnmatched: [], examples: 0 };
  assert.deepEqual(tableImportLines(r), [
    "Added 2 entries and 3 senses; 1 row was there already.",
    "Read Headword → lemma, English → gloss.",
    "Left out the column «Colour».",
    "Not added: «lari» names several entries; give those rows a homograph column.",
    "1 row has no word and was skipped.",
  ]);
  assert.equal(tableImportLines({ ...r, entries: 0, senses: 0, ambiguous: [], noLemma: 0, ignored: [] })[0], "Nothing new: every row is in the lexicon already.");
  const extras = tableImportLines({ ...r, ambiguous: [], noLemma: 0, ignored: [], domains: 2, examples: 1, domainsUnmatched: ["Moonbeams"] });
  assert.deepEqual(extras.slice(2), [
    "The new senses came with 2 links to semantic domains and 1 example sentence.",
    "No semantic domain is called «Moonbeams»: write a domain's number (2.1) or its name as the Lexicon lists it.",
  ]);
});

test("entries by category", () => {
  const rows = [
    { glosses: ["dog"], posOfSpeech: "n", used: true },
    { glosses: ["run"], posOfSpeech: "v ", used: true },
    { glosses: ["cat"], posOfSpeech: "n", used: false },
    { glosses: ["the"], posOfSpeech: "", used: true },
  ];
  assert.deepEqual(categoriesOf(rows), [{ category: "n", count: 2 }, { category: "v", count: 1 }]);
  assert.deepEqual(categoriesOf(rows.slice(0, 1), "pos:v"), [{ category: "n", count: 1 }, { category: "v", count: 0 }]);
  assert.deepEqual(rows.filter((r) => shows(r, "pos:n")).map((r) => r.glosses[0]), ["dog", "cat"]);
  assert.equal(shows(rows[1]!, "pos:v"), true);
});

test("loanwords and where they come from", () => {
  const r = (borrowedFrom: string) => ({ glosses: ["x"], posOfSpeech: "n", used: true, borrowedFrom });
  const rows = [r("Spanish"), r(" Spanish "), r("English"), r(""), r("")];
  assert.ok(shows(rows[0]!, "loans") && !shows(rows[3]!, "loans"));
  assert.deepEqual(loanSources(rows), [{ language: "Spanish", count: 2 }, { language: "English", count: 1 }]);
  assert.equal(loansLine(rows), "3 loanwords (60% of 5 entries): Spanish 2, English 1.");
  assert.match(loansLine([r("")]), /^No entry is marked/u);
});
