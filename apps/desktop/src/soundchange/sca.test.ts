import assert from "node:assert/strict";
import { test } from "node:test";
import { derivationsTsv, derive, EXAMPLE_RULES, expectedLine, lineThrough, pairsOf, parseRules, resultLine, targetName, wordsOf } from "./sca.ts";

const run = (rules: string, word: string) => derive(word, parseRules(rules).rules).output;

test("the notation reads", () => {
  const r = parseRules(EXAMPLE_RULES);
  assert.deepEqual(r.problems, []);
  assert.equal(r.rules.length, 4);
  assert.deepEqual(r.categories.get("V"), ["a", "e", "i", "o", "u"]);
  const bad = parseRules("V = a e\nnonsense\np t > b d g / _\na > e / i\n");
  assert.deepEqual(bad.problems.map((p) => p.line), [2, 3, 4]);
  assert.match(bad.problems[1]!.message, /2 sounds to change but 3/);
  assert.match(bad.problems[2]!.message, /needs one _/);
});

test("rules change words", () => {
  assert.equal(run("V = a e i o u\np t k > b d g / V_V", "pataka"), "padaga", "voicing between vowels, not word-initially");
  assert.equal(run("s/h/#_", "sasa"), "hasa", "slash notation and a word edge");
  assert.equal(run("V = a e i o u\nV > ∅ / _#", "kanu"), "kan");
  assert.equal(run("C = p t k\n∅ > ə / C_C", "aptka"), "apətəka", "insertion between consonants");
  assert.equal(run("C = p t k\ne > i / _(C)i", "eki ei eppi"), "iki ii eppi", "an optional element");
  assert.equal(run("a > e / _i // k_", "ai kai"), "ei kai", "an exception");
  assert.equal(run("ts > c\nt > d", "tsata"), "cada", "a sequence target, and rules in order");
  assert.equal(run("a > o\no > u", "ao"), "uu", "each rule sees the last one's output");
  assert.equal(run("a e > e a", "ae"), "ea", "simultaneous within a rule");
  assert.equal(run("e > i", "e̞e"), "e̞i", "a letter keeps its marks: e̞ is not e");
  assert.equal(run("V = a e\nC = p t\nD = b d\nC > D / V_", "apata"), "abada", "a category into a category");
  assert.equal(run("k > tʃ / _i, _e", "kiki ka ke"), "tʃitʃi ka tʃe", "several environments");
});

test("derivations, words and the line", () => {
  const rules = parseRules("a > o\no > u / _#\nx/y");
  const d = derive("ka", rules.rules);
  assert.deepEqual(d.steps, [{ line: 1, form: "ko" }, { line: 2, form: "ku" }]);
  assert.deepEqual(wordsOf("ka, ki\nka  ku;"), ["ka", "ki", "ku"]);
  assert.equal(resultLine([d, derive("i", rules.rules)], rules), "3 rules over 2 words: 1 changed.");
  assert.match(resultLine([], parseRules("bad")), /1 line does not read/);
  assert.equal(derivationsTsv([d]), "input\toutput\tsteps\nka\tku\t1: ko; 2: ku");
});

test("typed pairs give each word its attested reflex", () => {
  assert.deepEqual(pairsOf("kapa > kaba\npatu → padu\nsika\t∅\nlone words here\nkapa > kaba"), [
    { input: "kapa", expected: "kaba" },
    { input: "patu", expected: "padu" },
    { input: "sika", expected: "" },
    { input: "lone", expected: null },
    { input: "words", expected: null },
    { input: "here", expected: null },
  ]);
  const rules = parseRules("V = a e i o u\np t k > b d g / V_V").rules;
  const pairs = pairsOf("kapa > kaba\npatu > patu\nmisi");
  const d = pairs.map((p) => derive(p.input, rules));
  const e = pairs.map((p) => p.expected);
  assert.equal(expectedLine(d, e), " 1 of 2 words come out as expected (50%).");
  assert.equal(expectedLine(d, [null, null, null]), "");
  assert.equal(derivationsTsv(d, e).split("\n")[2], "patu\tpadu\tpatu\tno\t2: padu");
  assert.equal(derivationsTsv(d).split("\n")[0], "input\toutput\tsteps");
});

test("a line of a transcription goes through the rules word by word", () => {
  const r = parseRules("V = a e i o u\np t k > b d g / V_V\nu > ∅ / _#").rules;
  assert.equal(lineThrough("  kapa   satu  ", r), "kaba sad", "the rules run in order");
  assert.equal(lineThrough("", r), "");
  assert.equal(targetName("utterance"), "utterance after the sound changes");
});
