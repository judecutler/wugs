import assert from "node:assert/strict";
import { test } from "node:test";
import { createFieldController, type EditFn } from "./field.ts";

const flush = () => new Promise<void>((r) => setImmediate(r));

function harness(edit: EditFn) {
  const failures: string[] = [];
  const ctl = createFieldController({ edit, onFailure: (e) => failures.push(String(e)) });
  return { ctl, failures };
}

test("typing then commit writes the typed text once", async () => {
  const calls: [string, string][] = [];
  const { ctl } = harness(async (id, c) => (calls.push([id, c]), { content: c }));
  ctl.show("s1", "old");
  ctl.type("new");
  await ctl.commit();
  await ctl.commit();
  assert.deepEqual(calls, [["s1", "new"]]);
});

test("an untouched field writes nothing", async () => {
  const calls: string[] = [];
  const { ctl } = harness(async (_id, c) => (calls.push(c), { content: c }));
  ctl.show("s1", "old");
  await ctl.commit();
  assert.deepEqual(calls, []);
});

test("moving to another segment commits the one left", async () => {
  const calls: [string, string][] = [];
  const { ctl } = harness(async (id, c) => (calls.push([id, c]), { content: c }));
  ctl.show("s1", "");
  ctl.type("one");
  ctl.show("s2", "");
  await flush();
  assert.deepEqual(calls, [["s1", "one"]]);
  assert.equal(ctl.value, "");
});

test("an untouched field follows the store, a draft does not", () => {
  const { ctl } = harness(async (_id, c) => ({ content: c }));
  ctl.show("s1", "a");
  ctl.sync("b");
  assert.equal(ctl.value, "b");
  ctl.type("mine");
  ctl.sync("c");
  assert.equal(ctl.value, "mine");
});

test("revert returns to the stored text", () => {
  const { ctl } = harness(async (_id, c) => ({ content: c }));
  ctl.show("s1", "a");
  ctl.type("zzz");
  ctl.revert("a");
  assert.equal(ctl.value, "a");
});

const boom = () => Promise.reject(new Error("backend down"));

test("B1: a failed commit keeps the draft, and moving away and back shows it", async () => {
  const calls: string[] = [];
  let fail = true;
  const { ctl, failures } = harness(async (_id, c) => {
    if (fail) return boom();
    calls.push(c);
    return { content: c };
  });
  ctl.show("s1", "old");
  ctl.type("typed");
  ctl.show("s2", "other");
  await flush();
  assert.equal(failures.length, 1);
  ctl.show("s1", "old");
  assert.equal(ctl.value, "typed");
  fail = false;
  await ctl.commit();
  assert.deepEqual(calls, ["typed"]);
});

test("B1: a failed commit on the shown segment is not wiped by the next store refresh", async () => {
  const { ctl } = harness(boom);
  ctl.show("s1", "old");
  ctl.type("typed");
  await ctl.commit();
  ctl.sync("old");
  assert.equal(ctl.value, "typed");
  assert.equal(ctl.base, "old");
});

test("B1: a retry after a failure writes the draft again", async () => {
  let n = 0;
  const calls: string[] = [];
  const { ctl } = harness(async (_id, c) => {
    if (n++ === 0) return boom();
    calls.push(c);
    return { content: c };
  });
  ctl.show("s1", "old");
  ctl.type("typed");
  await ctl.commit();
  await ctl.commit();
  assert.deepEqual(calls, ["typed"]);
});

test("H1: going back before the store refresh shows the written text, and a second commit does not revert it", async () => {
  const calls: string[] = [];
  const { ctl } = harness(async (_id, c) => (calls.push(c), { content: c }));
  ctl.show("s1", "old");
  ctl.type("new");
  ctl.show("s2", "");
  await flush();
  ctl.show("s1", "old");
  assert.equal(ctl.value, "new");
  await ctl.commit();
  ctl.show("s2", "");
  await flush();
  assert.deepEqual(calls, ["new"]);
});

test("H1: going back while the edit is still in flight shows the typed text", async () => {
  let finish: () => void = () => {};
  const { ctl } = harness((_id, c) => new Promise((resolve) => (finish = () => resolve({ content: c }))));
  ctl.show("s1", "old");
  ctl.type("new");
  ctl.show("s2", "");
  ctl.show("s1", "old");
  assert.equal(ctl.value, "new");
  finish();
});

test("H1: the field shows the normalized text the edit returned", async () => {
  const { ctl } = harness(async () => ({ content: "NORMALIZED" }));
  ctl.show("s1", "old");
  ctl.type("raw");
  ctl.show("s2", "");
  await flush();
  ctl.show("s1", "old");
  assert.equal(ctl.value, "NORMALIZED");
});

test("H1: Esc reverts to the written text, not the stale stored one", async () => {
  const { ctl } = harness(async (_id, c) => ({ content: c }));
  ctl.show("s1", "old");
  ctl.type("new");
  ctl.show("s2", "");
  await flush();
  ctl.show("s1", "old");
  ctl.type("more");
  ctl.revert("old");
  assert.equal(ctl.value, "new");
});

test("H1: once the store echoes the written text, later store changes win", async () => {
  const { ctl } = harness(async (_id, c) => ({ content: c }));
  ctl.show("s1", "old");
  ctl.type("new");
  ctl.show("s2", "");
  await flush();
  ctl.show("s1", "new");
  ctl.show("s2", "");
  ctl.show("s1", "remote edit");
  assert.equal(ctl.value, "remote edit");
});

test("M3: text equal under NFC is not rewritten", async () => {
  const calls: string[] = [];
  const { ctl } = harness(async (_id, c) => (calls.push(c), { content: c }));
  ctl.show("s1", "café");
  ctl.type("café");
  await ctl.commit();
  assert.deepEqual(calls, []);
});

test("a draft marks its row and labels it with the draft text, until it saves", async () => {
  let fail = true;
  const { ctl } = harness(async (_id, c) => (fail ? boom() : { content: c }));
  ctl.show("s1", "old");
  ctl.type("typed");
  ctl.show("s2", "");
  await flush();
  assert.deepEqual(ctl.label("s1", "old"), { text: "typed", unsaved: true });
  assert.deepEqual(ctl.unsavedIds(), ["s1"]);
  fail = false;
  ctl.show("s1", "old");
  await ctl.commit();
  assert.deepEqual(ctl.label("s1", "old"), { text: "typed", unsaved: false });
  assert.deepEqual(ctl.unsavedIds(), []);
});

test("reverting drops the draft", async () => {
  const { ctl } = harness(boom);
  ctl.show("s1", "old");
  ctl.type("typed");
  await ctl.commit();
  ctl.revert("old");
  assert.equal(ctl.value, "old");
  assert.deepEqual(ctl.unsavedIds(), []);
});

test("M4: leaving commits the typed text and holds no segment", async () => {
  const calls: [string, string][] = [];
  const { ctl } = harness(async (id, c) => (calls.push([id, c]), { content: c }));
  ctl.show("s1", "old");
  ctl.type("typed");
  await ctl.leave();
  assert.deepEqual(calls, [["s1", "typed"]]);
  assert.equal(ctl.segmentId, null);
});

test("M4: a failure while leaving lands in the drafts", async () => {
  const { ctl } = harness(boom);
  ctl.show("s1", "old");
  ctl.type("typed");
  await ctl.leave();
  assert.deepEqual(ctl.label("s1", "old"), { text: "typed", unsaved: true });
});
