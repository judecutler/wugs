import assert from "node:assert/strict";
import { test } from "node:test";
import { breakdownChanged, formatBreakdown, joinedBreakdown, parseBreakdown, type ParsedMorph } from "./morphs.ts";

const ok = (input: string): ParsedMorph[] => {
  const r = parseBreakdown(input);
  assert.equal(r.ok, true, `${input}: ${r.ok ? "" : r.error}`);
  return r.ok ? r.morphs : [];
};
const m = (surface: string, type: ParsedMorph["type"]): ParsedMorph => ({ surface, type });

test("an unmarked word is one root", () => {
  assert.deepEqual(ok("köpek"), [m("köpek", "root")]);
  assert.deepEqual(ok(""), []);
  assert.deepEqual(ok("   "), []);
});

test("suffixes follow the root, prefixes come before it", () => {
  assert.deepEqual(ok("köpek-ler"), [m("köpek", "root"), m("ler", "suffix")]);
  assert.deepEqual(ok("köpek-ler-in"), [m("köpek", "root"), m("ler", "suffix"), m("in", "suffix")]);
  assert.deepEqual(ok("ka-köpek"), [m("ka", "prefix"), m("köpek", "root")]);
  assert.deepEqual(ok("ka-köpek-ler"), [m("ka", "prefix"), m("köpek", "root"), m("ler", "suffix")]);
  assert.deepEqual(ok("a-b-kitap"), [m("a", "prefix"), m("b", "prefix"), m("kitap", "root")]);
});

test("a marker at the edge of a piece says what it is", () => {
  assert.deepEqual(ok("-ler"), [m("ler", "suffix")]);
  assert.deepEqual(ok("ka-"), [m("ka", "prefix")]);
  assert.deepEqual(ok("-a-"), [m("a", "infix")]);
  assert.deepEqual(ok("un- do -ness"), [m("un", "prefix"), m("do", "root"), m("ness", "suffix")], "pieces written apart carry their own marker, whatever their length");
});

test("= marks a clitic", () => {
  assert.deepEqual(ok("=clitic"), [m("clitic", "clitic")]);
  assert.deepEqual(ok("köpek=ki"), [m("köpek", "root"), m("ki", "clitic")]);
  assert.deepEqual(ok("köpek-ler=ki"), [m("köpek", "root"), m("ler", "suffix"), m("ki", "clitic")]);
  assert.deepEqual(ok("ki="), [m("ki", "clitic")], "a trailing = writes a proclitic");
});

test("~ marks the reduplicant", () => {
  assert.deepEqual(ok("re~redup"), [m("re", "reduplication"), m("redup", "root")]);
  assert.deepEqual(ok("~re"), [m("re", "reduplication")]);
  assert.deepEqual(ok("ka~kaa-ler"), [m("ka", "reduplication"), m("kaa", "root"), m("ler", "suffix")]);
});

test("<…> marks an infix", () => {
  assert.deepEqual(ok("<in>fix"), [m("in", "infix"), m("fix", "root")]);
  assert.deepEqual(ok("s<um>ulat"), [m("s", "root"), m("um", "infix"), m("ulat", "root")]);
  assert.deepEqual(ok("<um>"), [m("um", "infix")]);
});

test("∅ is a zero morph, and never the root while a real piece is there", () => {
  assert.deepEqual(ok("köpek-∅"), [m("köpek", "root"), m("∅", "suffix")]);
  assert.deepEqual(ok("∅-köpek"), [m("∅", "prefix"), m("köpek", "root")]);
  assert.deepEqual(ok("∅"), [m("∅", "root")]);
  assert.deepEqual(ok("-∅"), [m("∅", "suffix")]);
});

test("a longer root wins, a tie goes to the first piece", () => {
  assert.deepEqual(ok("in-gel"), [m("in", "prefix"), m("gel", "root")]);
  assert.deepEqual(ok("ab-cd"), [m("ab", "root"), m("cd", "suffix")]);
});

test("separate words in one field are separate pieces", () => {
  assert.deepEqual(ok("kitap kapak"), [m("kitap", "root"), m("kapak", "root")]);
});

test("surfaces are NFC", () => {
  assert.deepEqual(ok("köpek"), [m("köpek", "root")]);
});

test("bad input says what is wrong and never throws", () => {
  for (const bad of ["a--b", "<in", "in>", "<>", "-", "=", "a-=b"]) {
    const r = parseBreakdown(bad);
    assert.equal(r.ok, false, bad);
    if (!r.ok) assert.notEqual(r.error, "");
  }
});

test("format writes every marker so it parses back to the same morphs", () => {
  const cases = ["köpek-ler", "ka-köpek-ler", "un- do -ness", "köpek=ki", "re~redup", "<in>fix", "∅-köpek", "köpek-∅", "kitap kapak"];
  for (const input of cases) {
    const morphs = ok(input);
    assert.deepEqual(ok(formatBreakdown(morphs)), morphs, input);
  }
  assert.equal(formatBreakdown(ok("un-do")), "un -do", "a tie breaks to the first piece, so this is root + suffix");
  assert.equal(formatBreakdown(ok("un- do")), "un- do", "writing the marker apart says prefix");
});

test("joined reads like a Leipzig word line", () => {
  assert.equal(joinedBreakdown(ok("köpek-ler")), "köpek-ler");
  assert.equal(joinedBreakdown(ok("ka-köpek-ler=ki")), "ka-köpek-ler=ki");
  assert.equal(joinedBreakdown(ok("re~redup")), "re~redup");
  assert.equal(joinedBreakdown(ok("<in>fix")), "<in>fix");
  assert.equal(joinedBreakdown(ok("kitap kapak")), "kitap kapak");
  assert.equal(joinedBreakdown([]), "");
});

test("breakdownChanged compares what would be written, not the spelling of the input", () => {
  const current = ok("köpek-ler");
  assert.equal(breakdownChanged("köpek-ler", current), false);
  assert.equal(breakdownChanged("köpek -ler", current), false);
  assert.equal(breakdownChanged("köpek-lar", current), true);
  assert.equal(breakdownChanged("köpek-", current), true);
  assert.equal(breakdownChanged("a--b", current), true, "an unparseable input is a change the editor must report");
  assert.equal(breakdownChanged("", []), false);
});
