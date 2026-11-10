import assert from "node:assert/strict";
import { test } from "node:test";
import { exactMatches, filterConcepts, isDone, nextTodo, progressLabel, progressShare } from "./logic.ts";

const c = (id: string, answered: boolean, skipped = false) => ({ id, skipped, answers: answered ? [{}] : [] });
const list = [c("a", true), c("b", false), c("c", false, true), c("d", false)];

test("done means answered or not applicable", () => {
  assert.deepEqual(list.map(isDone), [true, false, true, false]);
  assert.deepEqual(filterConcepts(list, "todo").map((x) => x.id), ["b", "d"]);
  assert.deepEqual(filterConcepts(list, "done").map((x) => x.id), ["a", "c"]);
  assert.equal(filterConcepts(list, "all").length, 4);
});

test("the next concept to do follows list order and wraps", () => {
  assert.equal(nextTodo(list, null)?.id, "b");
  assert.equal(nextTodo(list, "b")?.id, "d");
  assert.equal(nextTodo(list, "d")?.id, "b");
  assert.equal(nextTodo([c("x", true)], "x"), null);
  assert.equal(nextTodo([], null), null);
});

test("progress reads naturally", () => {
  assert.equal(progressLabel({ total: 100, answered: 37, skipped: 0 }), "37 of 100 answered");
  assert.equal(progressLabel({ total: 100, answered: 37, skipped: 2 }), "37 of 100 answered · 2 not applicable");
  assert.equal(progressShare({ total: 100, answered: 37, skipped: 3 }), 0.4);
  assert.equal(progressShare({ total: 0, answered: 0, skipped: 0 }), 0);
});

test("only exact lemmas are offered, case and NFC folded", () => {
  const rows = [{ lemma: "Su" }, { lemma: "sular" }, { lemma: "su" }];
  assert.deepEqual(exactMatches(rows, " SU "), [{ lemma: "Su" }, { lemma: "su" }]);
  assert.deepEqual(exactMatches(rows, ""), []);
  assert.deepEqual(exactMatches([{ lemma: "köpek" }], "köpek").length, 1);
});
