import assert from "node:assert/strict";
import { test } from "node:test";
import { fieldsLabel, needsNumbers, typedRaw, versionMeta } from "./logic.ts";

test("versions say whose and when", () => {
  const now = new Date(2026, 10, 9, 18, 0).getTime();
  assert.equal(versionMeta({ mine: true, atMs: new Date(2026, 10, 9, 14, 2).getTime() }, now), "this device · 14:02");
  assert.equal(versionMeta({ mine: false, atMs: new Date(2026, 10, 3, 9, 15).getTime() }, now), "another device · 3 Nov, 09:15");
  assert.equal(versionMeta({ mine: false, atMs: 0 }, now), "another device");
});

test("typed values are JSON strings in NFC", () => {
  assert.equal(typedRaw("köpek"), JSON.stringify("köpek"));
  assert.equal(typedRaw('a "b"'), '"a \\"b\\""');
});

test("look-alike versions get numbers", () => {
  assert.equal(needsNumbers(["home", "dwelling"]), false);
  assert.equal(needsNumbers(["home", "home"]), true);
  assert.equal(fieldsLabel(1), "1 field needs a decision");
  assert.equal(fieldsLabel(3), "3 fields need a decision");
});
