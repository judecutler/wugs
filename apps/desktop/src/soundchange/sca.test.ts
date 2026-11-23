import assert from "node:assert/strict";
import { test } from "node:test";
import { derivationsTsv, derive, EXAMPLE_RULES, parseRules, resultLine, wordsOf } from "./sca.ts";

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
