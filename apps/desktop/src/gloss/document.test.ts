import { test } from "node:test";
import assert from "node:assert/strict";
import { glossed, numbering, textDocument } from "./document.ts";
import type { Example } from "./example.ts";

const ex = (number: number, translation: string | null = "I count."): Example => ({
  number,
  translation,
  words: [{ form: "nikani", morphs: [{ surface: "ni-", type: "prefix", gloss: "1SG" }, { surface: "kani", type: "root", gloss: "count" }] }],
});

test("an expex document compiles as one file with every example", () => {
  const d = textDocument("Counting & more", [ex(1), ex(3)], "expex");
  assert.equal(d.extension, "tex");
  assert.match(d.contents, /\\usepackage\{expex\}/u);
  assert.match(d.contents, /\\section\*\{Counting \\& more\}/u, "the title is escaped");
  assert.equal(d.contents.match(/\\ex\n/gu)?.length, 2);
  assert.match(d.contents, /% \(3\)\n\\ex/u, "each example says which segment it is");
  assert.ok(d.contents.trimEnd().endsWith("\\end{document}"));
});

test("gb4e gets its package and the automath switch", () => {
  const d = textDocument("T", [ex(1)], "gb4e");
  assert.match(d.contents, /\\usepackage\{gb4e\}\n\\noautomath/u);
  assert.match(d.contents, /\\begin\{exe\}/u);
});

test("the Word document is HTML with a table per example, and plain text has blank lines between", () => {
  const w = textDocument("<T>", [ex(1), ex(2)], "word");
  assert.equal(w.extension, "html");
  assert.match(w.contents, /<title>&lt;T&gt;<\/title>/u);
  assert.equal(w.contents.match(/<table/gu)?.length, 2);
  const t = textDocument("T", [ex(1), ex(2, null)], "text");
  assert.equal(t.extension, "txt");
  assert.ok(t.contents.startsWith("T\n\n(1)"));
});

test("numbers follow the tier as Copy example numbers it, and unglossed sentences are not examples", () => {
  const n = numbering([{ id: "c", startMs: null }, { id: "b", startMs: 2000 }, { id: "a", startMs: 1000 }]);
  assert.deepEqual([n.get("a"), n.get("b"), n.get("c")], [1, 2, 3]);
  assert.equal(glossed({ number: 1, translation: null, words: [{ form: "x", morphs: [] }] }), false);
  assert.equal(glossed(ex(1)), true);
});
