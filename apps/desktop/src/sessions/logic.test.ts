import assert from "node:assert/strict";
import { test } from "node:test";
import { dateLabel, perPerson, sessionsLine, sessionsSheet, sessionsTsv, shownSessions, today, type SessionView } from "./logic.ts";

const s = (id: string, date: string, people: [string, string][], notes = "", place = "", method = ""): SessionView => ({ id, date, place, method, notes, people, texts: [] });
const log = [
  s("a", "2026-11-24", [["p1", "Ana"], ["p2", "Bén"]], "Body parts; ask «kani»?", "Village hall", "elicitation"),
  s("b", "2026-11-20", [["p1", "Ana"]], "Story of the fox"),
  s("c", "", []),
];

test("the log is filtered by words and by person", () => {
  assert.deepEqual(shownSessions(log, "ben", null).map((x) => x.id), ["a"], "accents ignored");
  assert.deepEqual(shownSessions(log, "fox", null).map((x) => x.id), ["b"]);
  assert.deepEqual(shownSessions(log, "", "p1").map((x) => x.id), ["a", "b"]);
  assert.deepEqual(shownSessions(log, "hall", "p2").map((x) => x.id), ["a"]);
});

test("dates and summaries in words", () => {
  assert.equal(dateLabel("2026-11-24"), "Tue 24 Nov 2026");
  assert.equal(dateLabel(""), "No date");
  assert.equal(today(new Date(2026, 0, 5)), "2026-01-05");
  assert.equal(sessionsLine(log, 3), "3 sessions on 2 days, with 2 people.");
  assert.equal(sessionsLine(log, 1), "3 sessions on 2 days, with 2 people. 1 shown.");
  assert.match(sessionsLine([], 0), /No sessions logged yet/u);
  assert.deepEqual(perPerson(log), [
    { id: "p1", name: "Ana", sessions: 2, last: "2026-11-24" },
    { id: "p2", name: "Bén", sessions: 1, last: "2026-11-24" },
  ]);
});

test("copied as a table and printed oldest first", () => {
  const tsv = sessionsTsv(log).split("\n");
  assert.equal(tsv[0], "date\tplace\tmethod\tpeople\ttexts\tnotes");
  assert.equal(tsv[1], "2026-11-24\tVillage hall\telicitation\tAna; Bén\t\tBody parts; ask «kani»?");
  const html = sessionsSheet(log, { language: "Tokana", generator: "wugs" });
  assert.ok(html.indexOf("Fri 20 Nov 2026") < html.indexOf("Tue 24 Nov 2026"), "oldest first");
  assert.ok(html.indexOf("Tue 24 Nov 2026") < html.indexOf("No date"), "undated last");
  assert.match(html, /<title>Tokana: fieldwork log<\/title>/u);
  assert.match(html, /Village hall · elicitation · Ana, Bén/u);
});
