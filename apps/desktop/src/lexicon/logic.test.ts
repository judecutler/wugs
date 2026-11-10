import assert from "node:assert/strict";
import { test } from "node:test";
import { domainMatches, filterDomains, glossLine, headword, nextHomograph, parseHomograph, sameLemmaNote, usageLabel } from "./logic.ts";

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
