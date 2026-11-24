import { test } from "node:test";
import assert from "node:assert/strict";
import { clock, spanOf, unclearLine, unclearSheet, type UnclearRow } from "./unclear.ts";

const row = (content: string, startMs: number | null, endMs: number | null, textTitle = "Story"): UnclearRow => ({ textId: "t", textTitle, tierId: "u", segmentId: "s", content, startMs, endMs, markers: ["(?)"] });

test("times read as a recording's clock", () => {
  assert.equal(clock(65_400), "1:05.4");
  assert.equal(clock(3_725_000), "1:02:05");
  assert.equal(spanOf(row("x", 1000, 2500)), "0:01.0–0:02.5");
  assert.equal(spanOf(row("x", null, null)), "untimed");
});

test("the line and the sheet", () => {
  assert.match(unclearLine({ checked: 4, total: 0, rows: [] }), /^None of 4 transcribed segments/);
  assert.equal(unclearLine({ checked: 4, total: 1, rows: [row("a (?)", 0, 1)] }), "1 of 4 transcribed segments has a passage marked unclear. Print the list to go through with a speaker at the next session.");
  const html = unclearSheet([row("the <fox> (?)", 0, 900), row("xxx ran", 1000, 1900), row("[unclear]", null, null, "Song")], { language: "Kanu", date: "2026-11-24", generator: "wugs" });
  assert.match(html, /<title>Kanu: unclear passages to check<\/title>/);
  assert.equal((html.match(/<h2>/g) ?? []).length, 2);
  assert.ok(html.includes("the &lt;fox&gt; (?)"));
  assert.ok(html.includes("0:00.0–0:00.9"));
});
