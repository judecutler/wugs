import { test } from "node:test";
import assert from "node:assert/strict";
import { localDay } from "./dates.ts";

test("the local calendar day, zero-padded", () => {
  assert.equal(localDay(new Date(2026, 0, 5, 23, 59)), "2026-01-05");
  assert.equal(localDay(new Date(2026, 10, 12, 0, 1)), "2026-11-12");
});
