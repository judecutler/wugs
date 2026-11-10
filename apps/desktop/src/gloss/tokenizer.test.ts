import assert from "node:assert/strict";
import { test } from "node:test";
import { forms, tokenize } from "./tokenizer.ts";

test("splits on whitespace of every kind and drops empty chunks", () => {
  assert.deepEqual(forms("  köpekler\tgeldi\n dün  "), ["köpekler", "geldi", "dün"]);
  assert.deepEqual(forms(""), []);
  assert.deepEqual(forms("   "), []);
});

test("edge punctuation is stripped and remembered, not lost", () => {
  assert.deepEqual(tokenize("«Köpekler», geldi."), [
    { form: "Köpekler", lead: "«", trail: "»," },
    { form: "geldi", lead: "", trail: "." },
  ]);
  assert.deepEqual(tokenize("(wug) !!"), [{ form: "wug", lead: "(", trail: ")!!" }]);
  assert.deepEqual(tokenize("— wug"), [{ form: "wug", lead: "—", trail: "" }], "a leading dash chunk joins the next word");
  assert.deepEqual(tokenize("a ... b"), [
    { form: "a", lead: "", trail: "..." },
    { form: "b", lead: "", trail: "" },
  ]);
});

test("morph markers stay inside and at the edges of words", () => {
  assert.deepEqual(forms("köpek-ler =ki re~redup <in>fix s<um>ulat"), ["köpek-ler", "=ki", "re~redup", "<in>fix", "s<um>ulat"]);
  assert.deepEqual(forms("ka- -ler"), ["ka-", "-ler"]);
  assert.deepEqual(forms("a - b"), ["a", "b"], "a lone hyphen is punctuation");
});

test("interior punctuation is part of the word", () => {
  assert.deepEqual(forms("don't ka.pa 3.5 e-mail"), ["don't", "ka.pa", "3.5", "e-mail"]);
});

test("IPA: modifier letters, stress, length, tone letters and combining marks stay", () => {
  assert.deepEqual(forms("[ˈkœpɛk.lɛɾ] ʔaˈsaː /tʃʼa˥˩/ ã̰ t͡ʃ"), ["ˈkœpɛk.lɛɾ", "ʔaˈsaː", "tʃʼa˥˩", "ã̰", "t͡ʃ"]);
  assert.equal(tokenize("[ˈkœpɛk.lɛɾ]")[0]!.lead, "[");
  assert.deepEqual(forms("ǃxóõ ǂkhomani ʘ"), ["ǃxóõ", "ǂkhomani", "ʘ"], "click letters are letters");
});

test("glottal stops and ejectives at the edge of a word belong to it", () => {
  assert.deepEqual(forms("ʼayo kaʼ 'ayo ka' ʔ"), ["ʼayo", "kaʼ", "'ayo", "ka'", "ʔ"]);
});

test("a word in a quote pair loses the quotes, a lone edge apostrophe does not", () => {
  assert.deepEqual(tokenize("'wug'"), [{ form: "wug", lead: "'", trail: "'" }]);
  assert.deepEqual(tokenize("‘wug’,"), [{ form: "wug", lead: "‘", trail: "’," }]);
  assert.deepEqual(forms("ka'a 'wug"), ["ka'a", "'wug"]);
});

test("forms come out in NFC", () => {
  const out = forms("köpek");
  assert.deepEqual(out, ["köpek"]);
});

test("tone digits and numerals are word material", () => {
  assert.deepEqual(forms("ma1 ma55, 2024."), ["ma1", "ma55", "2024"]);
});

test("a script without spaces comes back as one word", () => {
  assert.deepEqual(forms("私は犬を見た"), ["私は犬を見た"]);
});
