import assert from "node:assert/strict";
import { test } from "node:test";
import { decideRedraw, sameContent } from "./redraw.ts";

const text = (content: string) => ({ id: "txt_1", segments: [{ id: "seg_1", content }] });

test("the first draw always happens, even while editing", () => {
  assert.equal(decideRedraw(null, text("a"), true, sameContent), "draw");
});

test("a refetch of the same content does not redraw", () => {
  assert.equal(decideRedraw({ value: text("a") }, text("a"), false, sameContent), "same");
  assert.equal(decideRedraw({ value: text("a") }, text("a"), true, sameContent), "same");
});

test("new content waits while an input in the panel has focus", () => {
  assert.equal(decideRedraw({ value: text("a") }, text("b"), true, sameContent), "defer");
  assert.equal(decideRedraw({ value: text("a") }, text("b"), false, sameContent), "draw");
});

test("without a content comparison, identity decides", () => {
  const t = text("a");
  assert.equal(decideRedraw({ value: t }, t, false), "same");
  assert.equal(decideRedraw({ value: t }, text("a"), false), "draw");
});
