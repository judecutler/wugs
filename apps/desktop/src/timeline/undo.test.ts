import { test } from "node:test";
import assert from "node:assert/strict";
import { checkOf, COALESCE_MS, edgeEntry, UndoHistory, UndoStack, type SegmentState } from "./undo.ts";

/** A fake document: undo and redo write values back, like the commands do. */
function counter() {
  const writes: string[] = [];
  const entry = (label: string, from: number, to: number, doc: { v: number }) => ({
    label,
    undo: async () => {
      doc.v = from;
      writes.push(`${label}<-${from}`);
    },
    redo: async () => {
      doc.v = to;
      writes.push(`${label}->${to}`);
    },
  });
  return { writes, entry };
}

test("undo and redo walk the edits back and forth", async () => {
  const s = new UndoStack();
  const doc = { v: 2 };
  const { writes, entry } = counter();
  s.push(entry("a", 0, 1, doc));
  s.push(entry("b", 1, 2, doc));
  assert.equal(s.undoLabel(), "b");
  assert.equal(await s.undo(), "b");
  assert.equal(doc.v, 1);
  assert.equal(await s.undo(), "a");
  assert.equal(doc.v, 0);
  assert.equal(await s.undo(), null, "nothing left");
  assert.equal(s.redoLabel(), "a");
  assert.equal(await s.redo(), "a");
  assert.equal(await s.redo(), "b");
  assert.equal(doc.v, 2);
  assert.deepEqual(writes, ["b<-1", "a<-0", "a->1", "b->2"]);
});

test("a new edit clears redo", async () => {
  const s = new UndoStack();
  const doc = { v: 0 };
  const { entry } = counter();
  s.push(entry("a", 0, 1, doc));
  await s.undo();
  assert.ok(s.canRedo());
  const c = entry("c", 0, 5, doc);
  s.push(c);
  assert.ok(!s.canRedo());
  assert.ok(s.isNewest(c));
  s.push(entry("d", 5, 6, doc));
  assert.ok(!s.isNewest(c), "a later edit is newest now");
  assert.equal(s.undoLabel(), "d");
});

test("a failed undo drops the entry and passes the error on", async () => {
  const s = new UndoStack();
  s.push({ label: "gone", undo: () => Promise.reject(new Error("no such segment")), redo: async () => {} });
  await assert.rejects(s.undo(), /no such segment/);
  assert.ok(!s.canUndo() && !s.canRedo());
});

test("one undo at a time: a second press while one runs does nothing", async () => {
  const s = new UndoStack();
  let release: () => void = () => {};
  s.push({ label: "slow", undo: () => new Promise<void>((r) => (release = r)), redo: async () => {} });
  s.push({ label: "slow2", undo: () => new Promise<void>((r) => (release = r)), redo: async () => {} });
  const first = s.undo();
  assert.ok(!s.canUndo());
  assert.equal(await s.undo(), null);
  release();
  assert.equal(await first, "slow2");
  assert.ok(s.canUndo());
});

test("nudges with one key fold into one entry while they come quickly", async () => {
  const s = new UndoStack();
  const doc = { v: 0 };
  const { entry } = counter();
  s.push(entry("nudge", 0, 10, doc), "seg1:start", 1000);
  s.push(entry("nudge", 10, 20, doc), "seg1:start", 1500);
  s.push(entry("nudge", 20, 30, doc), "seg1:start", 1900);
  s.push(entry("nudge", 30, 40, doc), "seg1:start", 1900 + COALESCE_MS + 1);
  doc.v = 40;
  await s.undo();
  assert.equal(doc.v, 30, "the late nudge is its own entry");
  await s.undo();
  assert.equal(doc.v, 0, "the quick run goes back in one step");
  await s.redo();
  assert.equal(doc.v, 30, "and comes back in one step");
});

test("history keeps a stack per text and forgets another project's", () => {
  const h = new UndoHistory();
  const a = h.stack("p1", "t1");
  a.push({ label: "x", undo: async () => {}, redo: async () => {} });
  assert.equal(h.stack("p1", "t1"), a);
  assert.ok(!h.stack("p1", "t2").canUndo(), "another text has its own stack");
  assert.ok(h.stack("p1", "t1").canUndo());
  assert.ok(!h.stack("p2", "t1").canUndo(), "another project starts empty");
  assert.ok(!h.stack("p1", "t1").canUndo(), "and switching back does not bring old edits back");
});

test("folded edge moves undo to the first position and check the last one", async () => {
  // A fake segment edge with the command's check: refused unless it is where expected.
  let edge = 0;
  const writes: string[] = [];
  const write = async (ms: number, expectMs: number) => {
    if (edge !== expectMs) throw new Error("That segment was edited since");
    edge = ms;
    writes.push(`${expectMs}->${ms}`);
  };
  const s = new UndoStack();
  for (const [from, to, at] of [[0, 10, 1000], [10, 20, 1200], [20, 30, 1400]] as const) {
    edge = to;
    s.push(edgeEntry("nudge", from, to, write), "seg1:end", at);
  }
  await s.undo();
  assert.equal(edge, 0);
  await s.redo();
  assert.equal(edge, 30);
  assert.deepEqual(writes, ["30->0", "0->30"], "one write each way, each checking where the edge was");
  edge = 55; // moved since, in another panel
  await assert.rejects(s.undo(), /edited since/);
  assert.equal(edge, 55, "nothing written");
  assert.ok(!s.canUndo(), "the refused entry is dropped");
});

test("a check covers the parts asked for, all by default", () => {
  const st: SegmentState = { id: "seg1", deleted: false, startMs: 100, endMs: 900, content: "köpek", words: 1 };
  assert.deepEqual(checkOf(st), { id: "seg1", deleted: false, startMs: 100, endMs: 900, content: "köpek", words: 1 });
  assert.deepEqual(checkOf(st, ["endMs"]), { id: "seg1", endMs: 900 });
  assert.deepEqual(checkOf({ ...st, startMs: null }, ["startMs", "deleted"]), { id: "seg1", deleted: false }, "an untimed edge is not checked");
});
