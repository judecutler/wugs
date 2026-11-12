import assert from "node:assert/strict";
import { test } from "node:test";
import type { SearchRow } from "../api.ts";
import { clockTime, groupRows, moveActive, navigable, navTargets, whereLabel } from "./logic.ts";

const row = (over: Partial<SearchRow>): SearchRow => ({
  target: "x", kind: "segment", field: "content", snippet: "", textId: null, textTitle: null, tierId: null, tierName: null, startMs: null, endMs: null, ...over,
});
const seg = (target: string, textId: string, title: string, startMs = 0) =>
  row({ target, textId, textTitle: title, tierId: "t", tierName: "Utterance", startMs });
const lex = (target: string, kind = "lexeme") => row({ target, kind, field: "lemma" });

test("groups by text in order of best hit, with Lexicon and Other after their first hit", () => {
  const groups = groupRows([seg("a", "T2", "Two"), lex("l1"), seg("b", "T1", "One"), seg("c", "T2", "Two"), row({ target: "tok", kind: "token" }), lex("l2", "sense")]);
  assert.deepEqual(groups.map((g) => g.title), ["Two", "Lexicon", "One", "Other"]);
  assert.deepEqual(groups[0].rows.map((r) => r.target), ["a", "c"], "rank order kept within a group");
  assert.deepEqual(groups[1].rows.map((r) => r.target), ["l1", "l2"]);
});

test("a text without a title still gets a header", () => {
  assert.equal(groupRows([seg("a", "T1", "")])[0].title, "Untitled text");
});

test("only rows naming a text are navigable, and arrow keys walk them in display order", () => {
  const groups = groupRows([seg("a", "T2", "Two"), lex("l1"), seg("b", "T1", "One"), seg("c", "T2", "Two")]);
  assert.equal(navigable(lex("l1")), false);
  assert.deepEqual(navTargets(groups).map((r) => r.target), ["a", "c", "b"], "grouped order, lexicon skipped");
});

test("moveActive stops at the ends and starts from the top", () => {
  assert.equal(moveActive(-1, "ArrowDown", 3), 0);
  assert.equal(moveActive(0, "ArrowDown", 3), 1);
  assert.equal(moveActive(2, "ArrowDown", 3), 2);
  assert.equal(moveActive(1, "ArrowUp", 3), 0);
  assert.equal(moveActive(0, "ArrowUp", 3), 0);
  assert.equal(moveActive(-1, "ArrowUp", 3), 0);
  assert.equal(moveActive(0, "ArrowDown", 0), -1);
});

test("clock time is m:ss", () => {
  assert.equal(clockTime(0), "0:00");
  assert.equal(clockTime(5400), "0:05");
  assert.equal(clockTime(65_000), "1:05");
  assert.equal(clockTime(3_725_000), "62:05");
});

test("a segment hit is labelled with time and tier, others are not", () => {
  assert.equal(whereLabel(seg("a", "T", "t", 65_000)), "1:05 · Utterance");
  assert.equal(whereLabel(row({ kind: "segment", tierName: "Gloss" })), "Gloss");
  assert.equal(whereLabel(lex("l")), "");
});
