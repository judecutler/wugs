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
