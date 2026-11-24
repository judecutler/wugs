import assert from "node:assert/strict";
import { test } from "node:test";
import { applyShortcuts, EXAMPLE_TABLE, keymanId, keymanSource, parseShortcuts, shortcutsLine } from "./shortcuts.ts";

test("a table reads line by line, longest first, problems named", () => {
  const { shortcuts, problems } = parseShortcuts("# mine\n;n ŋ\n;ng ŋg\n\nbad\n;n ɲ\nabcdefghij x\n");
  assert.deepEqual(shortcuts, [
    { from: ";ng", to: "ŋg" },
    { from: ";n", to: "ŋ" },
  ]);
  assert.deepEqual(problems, [
    'Line 5: write what you type, a space, and what it becomes, such as ";n ŋ".',
    "Line 6: «;n» is already in the table.",
    "Line 7: at most 8 characters on each side.",
  ]);
  assert.equal(parseShortcuts(EXAMPLE_TABLE).problems.length, 0);
});

test("the shortcut just typed becomes its letter, the caret after it", () => {
  const { shortcuts } = parseShortcuts(";n ŋ\n;ng ŋg\nb' ɓ\né é");
  assert.deepEqual(applyShortcuts("ka;n", 4, shortcuts), { value: "kaŋ", caret: 3 });
  assert.deepEqual(applyShortcuts("ka;ng", 5, shortcuts), { value: "kaŋg", caret: 4 }, "the longest wins");
  assert.deepEqual(applyShortcuts("b'a", 2, shortcuts), { value: "ɓa", caret: 1 }, "mid-field, the rest kept");
  assert.equal(applyShortcuts("kan", 3, shortcuts), null);
  assert.equal(applyShortcuts("", 0, shortcuts), null);
});

test("the toolbar line", () => {
  assert.equal(shortcutsLine(0), "No typing shortcuts yet.");
  assert.equal(shortcutsLine(1), "1 typing shortcut on, for everyone on this project.");
});

test("the shortcuts as a Keyman keyboard", () => {
  const { shortcuts } = parseShortcuts(";n ŋ\nb' ɓ\na:: aː\nx ʃ\nnŋ ŋŋ");
  const { source, left } = keymanSource(shortcuts, "Kanu");
  assert.match(source, /store\(&NAME\) 'Kanu \(wugs\)'/u);
  assert.match(source, /store\(&MnemonicLayout\) '1'/u);
  assert.match(source, /^U\+003B \+ 'n' > U\+014B {2}c ;n → ŋ$/mu);
  assert.match(source, /^U\+0062 \+ "'" > U\+0253/mu, "an apostrophe key in double quotes");
  assert.match(source, /^U\+0061 U\+003A \+ ':' > U\+0061 U\+02D0/mu);
  assert.match(source, /^\+ 'x' > U\+0283/mu, "one key alone");
  assert.deepEqual(left, [{ from: "nŋ", to: "ŋŋ" }], "ŋ is no key on a keyboard");
  assert.equal(keymanId("Kanu"), "wugs_kanu");
  assert.equal(keymanId("Türkçe 2"), "wugs_turkce_2");
  assert.equal(keymanId("ʔ"), "wugs_project");
  assert.equal(keymanId("9 lives"), "wugs_x9_lives");
});
