import assert from "node:assert/strict";
import { test } from "node:test";
import { IPA_GROUPS, projectGroup } from "./ipa.ts";

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

test("the project's own letters a keyboard does not type come first, commonest first", () => {
  const row = (ch: string, kind: string, inTexts: number, inLexicon = 0, script = "Latin") => ({ ch, name: ch, kind, script, inTexts, inLexicon });
  const g = projectGroup([row("a", "letter", 900), row("ŋ", "letter", 40), row("\u0301", "mark", 60, 0, "Inherited"), row("ɓ", "letter", 0, 3), row("ʼ", "letter", 0, 0, "Common"), row("2", "digit", 5), row("\u00a0", "invisible", 9), row("а", "letter", 1, 0, "Cyrillic")], "Latin");
  assert.ok(g);
  assert.equal(g.label, "This project");
  assert.deepEqual(g.symbols.map((s) => s.ch), ["\u0301", "ŋ", "ɓ"]);
  assert.equal(g.symbols[0]!.combining, true);
  assert.equal(projectGroup([row("a", "letter", 3)], "Latin"), null);
});
