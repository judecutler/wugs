import { test } from "node:test";
import assert from "node:assert/strict";
import { dayLabel, shift, workColumns, worklogLine, worklogTsv, type Day } from "./worklog.ts";

const day = (date: string, over: Partial<Day> = {}): Day => ({ date, segments: 0, transcribed: 0, translated: 0, analysed: 0, glossed: 0, entries: 0, senses: 0, notes: 0, importedTexts: 0, importedEntries: 0, machine: 0, ops: 1, devices: 1, ...over });

test("days and columns", () => {
  assert.equal(shift("2026-11-24", -6), "2026-11-18");
  assert.equal(shift("2026-03-01", -1), "2026-02-28");
  assert.equal(dayLabel("2026-11-24"), "Tue 24 Nov 2026");
  const days = [day("2026-11-24", { transcribed: 12, glossed: 30 }), day("2026-11-20", { transcribed: 3, entries: 1 }), day("2026-10-01", { importedTexts: 2 })];
  assert.deepEqual(workColumns(days).map((c) => c.key), ["transcribed", "glossed", "entries", "importedTexts"]);
  assert.equal(worklogLine(days, "2026-11-24"), "In the last seven days: 15 sentences transcribed, 30 glosses, 1 new entry, on 2 days. The table lists the 3 latest days with any work, each thing counted once a day however often it was edited.");
  assert.match(worklogLine([day("2026-10-01")], "2026-11-24"), /^Nothing was done in the last seven days\./u);
  assert.equal(worklogLine([], "2026-11-24"), "Nothing has been written in this project yet.");
  assert.equal(worklogTsv(days).split("\n")[1], "2026-11-24\t0\t12\t0\t0\t30\t0\t0\t0\t0\t0\t0\t1");
});
