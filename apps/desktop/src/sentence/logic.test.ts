import { test } from "node:test";
import assert from "node:assert/strict";
import { arcs, checkLine, columns, headChoices, nextUnchecked, rootOf, type WordRow } from "./logic.ts";

const w = (form: string, head: number | null, deprel = ""): WordRow => ({ token: form, form, lemma: "", upos: "", xpos: "", feats: "", head, deprel });

test("arcs nest: an arc rides above the arcs it spans", () => {
  // The(1) big(2) dog(3) barked(4): The→dog, big→dog, dog→barked; barked is the root.
  const s = [w("The", 3, "det"), w("big", 3, "amod"), w("dog", 4, "nsubj"), w("barked", 0, "root")];
  const a = arcs(s);
  assert.equal(a.length, 3, "the root has no arc");
  const by = (dep: number) => a.find((x) => x.dep === dep)!;
  assert.equal(by(1).level, 1, "big→dog is the shortest");
  assert.equal(by(0).level, 2, "The→dog spans big→dog");
  assert.equal(by(2).level, 1, "dog→barked spans nothing");
  assert.equal(rootOf(s), 3);
});

test("heads outside the sentence or on the word itself draw nothing", () => {
  assert.deepEqual(arcs([w("a", 5), w("b", 2), w("c", null)]), []);
  assert.equal(rootOf([w("a", null)]), null);
});

test("columns leave room for the longer of form and tag", () => {
  const c = columns([{ ...w("I", 0), upos: "PRON" }, w("understand", 1)]);
  assert.equal(c[0]!.width, 34, "short words keep a minimum");
  assert.equal(c[1]!.x, 48);
  assert.equal(c[1]!.width, 80);
});

test("a word cannot choose itself as head", () => {
  const choices = headChoices([w("a", 0), w("b", 1)], 1);
  assert.deepEqual(choices.map((c) => c.value), ["", "0", "1"]);
});

test("the next sentence to check follows the one on screen and wraps round", () => {
  const v = { analysed: 4, unchecked: ["b", "d"], sentences: ["a", "b", "c", "d", "e"] };
  assert.equal(nextUnchecked(v, "a"), "b");
  assert.equal(nextUnchecked(v, "b"), "d");
  assert.equal(nextUnchecked(v, "c"), "d");
  assert.equal(nextUnchecked(v, "e"), "b", "wraps round");
  assert.equal(nextUnchecked(v, null), "b");
  assert.equal(nextUnchecked({ ...v, unchecked: ["b"] }, "b"), "b", "the last one left");
  assert.equal(nextUnchecked({ ...v, unchecked: [] }, "a"), null);
  assert.equal(checkLine(v), "2 of 4 tagged sentences still to check.");
  assert.equal(checkLine({ ...v, unchecked: [] }), "All 4 tagged sentences are checked.");
  assert.equal(checkLine({ analysed: 0, unchecked: [], sentences: [] }), "");
});
