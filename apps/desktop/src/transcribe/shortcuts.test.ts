import assert from "node:assert/strict";
import { test } from "node:test";
import { applyShortcuts, EXAMPLE_TABLE, parseShortcuts, shortcutsLine } from "./shortcuts.ts";

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
