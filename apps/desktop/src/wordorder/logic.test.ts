import assert from "node:assert/strict";
import { test } from "node:test";
import { answerNote, countsLine, grambankAnswers, orderLine, orderTsv, verdict, type Pattern } from "./logic.ts";

const p = (id: string, counts: [string, string, number][]): Pattern => ({ id, title: id, orders: counts.map(([label, code, count]) => ({ label, code, count, example: null })) });

test("the dominant order follows WALS's twice-as-common rule", () => {
  assert.deepEqual(verdict(p("83A", [["OV", "1", 2], ["VO", "2", 40]])), { label: "VO", code: "2", why: "VO is at least twice as common as any other order." });
  assert.equal(verdict(p("83A", [["OV", "1", 15], ["VO", "2", 20]])).code, "3");
  assert.equal(verdict(p("88A", [["Demonstrative-Noun", "1", 15], ["Noun-Demonstrative", "2", 20]])).label, "Mixed");
  assert.equal(verdict(p("143A", [["NegV", "1", 15], ["VNeg", "2", 20]])).code, null, "143A has no code for it");
  assert.equal(verdict(p("82A", [["SV", "1", 3], ["VS", "2", 0]])).label, "Too few to tell");
  assert.equal(verdict(p("82A", [["SV", "1", 0], ["VS", "2", 0]])).label, "Not found");
  const sov = p("81A", [["SOV", "1", 3], ["SVO", "2", 40], ["VSO", "3", 0]]);
  assert.equal(verdict(sov).label, "SVO");
  assert.equal(countsLine(sov), "SOV 3, SVO 40");
  assert.equal(answerNote(sov, 120), "Counted in 120 parsed sentences: SOV 3, SVO 40.");
  assert.match(orderLine({ parsed: 0, patterns: [] }), /^No sentence has a dependency parse yet/);
  assert.match(orderLine({ parsed: 120, patterns: [sov, p("82A", [["SV", "1", 1]])] }), /^Counted in 120 parsed sentences; 1 of 2 features/);
  assert.equal(orderTsv({ parsed: 1, patterns: [sov] }).split("\n")[1], "81A\t81A\tSOV 3, SVO 40\tSVO\t2");
});

test("an order's verdict answers the Grambank questions that ask it", () => {
  const pattern = (id: string, counts: [string, string, number][]) => ({ id, title: id, orders: counts.map(([label, code, count]) => ({ label, code, count, example: null })) });
  assert.deepEqual(grambankAnswers(pattern("89A", [["Numeral-Noun", "1", 9], ["Noun-Numeral", "2", 1]])), [["GB024", "1"]]);
  assert.deepEqual(grambankAnswers(pattern("89A", [["Numeral-Noun", "1", 6], ["Noun-Numeral", "2", 5]])), [["GB024", "3"]], "no dominant order is Grambank's both");
  assert.deepEqual(grambankAnswers(pattern("81A", [["SOV", "1", 12], ["SVO", "2", 2]])), [["GB131", "0"], ["GB132", "0"], ["GB133", "1"]]);
  assert.deepEqual(grambankAnswers(pattern("85A", [["Postpositions", "1", 8], ["Prepositions", "2", 0]])), [["GB074", "0"], ["GB075", "1"]]);
  assert.deepEqual(grambankAnswers(pattern("85A", [["Postpositions", "1", 1], ["Prepositions", "2", 0]])), [], "too few to say");
  assert.deepEqual(grambankAnswers(pattern("90A", [["Noun-Relative clause", "1", 9], ["Relative clause-Noun", "2", 0]])), []);
  assert.deepEqual(grambankAnswers(pattern("82A", [["SV", "1", 2], ["VS", "2", 0]])), [], "under the minimum");
});
