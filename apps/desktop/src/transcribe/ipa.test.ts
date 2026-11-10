import assert from "node:assert/strict";
import { test } from "node:test";
import { IPA_GROUPS } from "./ipa.ts";

const all = IPA_GROUPS.flatMap((g) => g.symbols);

test("no symbol appears twice", () => {
  const seen = new Set<string>();
  for (const s of all) {
    assert.equal(seen.has(s.ch), false, `duplicate ${s.ch} (U+${s.ch.codePointAt(0)?.toString(16)})`);
    seen.add(s.ch);
  }
});

test("every entry is one code point with a name", () => {
  for (const s of all) {
    assert.equal([...s.ch].length, 1, `${s.name} is not a single code point`);
    assert.notEqual(s.name, "", `U+${s.ch.codePointAt(0)?.toString(16)} has no name`);
  }
});

test("combining entries are combining marks, and only they are", () => {
  for (const s of all) assert.equal(/^\p{Mn}$/u.test(s.ch), s.combining === true, `${s.name} (U+${s.ch.codePointAt(0)?.toString(16)})`);
});

test("groups have unique ids and are not empty", () => {
  assert.equal(new Set(IPA_GROUPS.map((g) => g.id)).size, IPA_GROUPS.length);
  for (const g of IPA_GROUPS) assert.ok(g.symbols.length > 0, g.id);
});

test("plain() names with spaces survive the split", () => {
  assert.equal(all.find((s) => s.ch === "ə")?.name, "schwa");
  assert.equal(all.find((s) => s.ch === "i")?.name, "close front unrounded");
});
