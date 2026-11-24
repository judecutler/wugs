import assert from "node:assert/strict";
import { test } from "node:test";
import { ROLES, termLabel } from "./logic.ts";

test("every term the engine offers reads in words", () => {
  for (const t of ["archive-ok", "web-ok", "community-only", "AI-ok"]) assert.notEqual(termLabel(t), t);
  assert.equal(termLabel("x-custom"), "x-custom", "an unknown term shows as written");
});

test("roles are unique", () => {
  assert.equal(new Set(ROLES).size, ROLES.length);
});

test("people as a table for an archive", async () => {
  const { detailKeys, detailLabel, peopleTsv } = await import("./logic.ts");
  assert.equal(detailLabel("born"), "Born (year)");
  assert.equal(detailLabel("clan_name"), "clan name");
  assert.deepEqual(detailKeys([["clan_name", "x"]]).slice(-2), ["notes", "clan_name"]);
  const tsv = peopleTsv(
    [
      { name: "Ana Reyes", roles: ["speaker"], details: [["born", "1958"], ["languages", "Kanu, Tok Pisin"]], consents: [{ covers: "p1", terms: [["archive-ok", true], ["web-ok", false], ["AI-ok", null]], date: "2026-11-11" }], deleted: false },
      { name: "Ben", roles: [], details: [], consents: [], deleted: false },
      { name: "Gone", roles: [], details: [["born", "1900"]], consents: [], deleted: true },
    ],
    (id) => (id === "p1" ? "The whole project" : id),
  ).split("\n");
  assert.equal(tsv[0], "Name\tRoles\tBorn (year)\tLanguages\tConsent");
  assert.equal(tsv[1], "Ana Reyes\tspeaker\t1958\tKanu, Tok Pisin\tThe whole project (2026-11-11): Deposit in an archive: yes, Publish online: no");
  assert.equal(tsv[2], "Ben\t\t\t\t");
  assert.equal(tsv.length, 4, "a deleted person is left out");
});
