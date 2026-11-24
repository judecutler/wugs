import assert from "node:assert/strict";
import { test } from "node:test";
import { nextToDo, progressLine, shownPrompts, type PromptRow } from "./logic.ts";

const row = (id: string, answered: boolean): PromptRow => ({ id, number: Number(id), prompt: `p${id}`, answers: answered ? [{ segmentId: `s${id}`, tierId: "u", content: "x" }] : [] });
const rows = [row("1", true), row("2", false), row("3", true), row("4", false)];

test("progress and what comes next", () => {
  assert.equal(progressLine({ id: "a", name: "A", total: 4, answered: 2, textId: null }), "2 of 4 prompts answered.");
  assert.equal(progressLine({ id: "a", name: "A", total: 4, answered: 4, textId: null }), "Every one of the 4 prompts has an answer.");
  assert.deepEqual(shownPrompts(rows, true).map((r) => r.id), ["2", "4"]);
  assert.equal(shownPrompts(rows, false).length, 4);
  assert.equal(nextToDo(rows, null)?.id, "2");
  assert.equal(nextToDo(rows, "2")?.id, "4");
  assert.equal(nextToDo(rows, "4")?.id, "2", "wraps round");
  assert.equal(nextToDo([row("1", true)], "1"), null);
});
