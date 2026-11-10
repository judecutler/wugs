import assert from "node:assert/strict";
import test from "node:test";
import { idle, isLive, step, type Take, type TakeEvent } from "./take.ts";

const run = (events: TakeEvent[], from: Take = idle): Take => events.reduce(step, from);
const pending: Take = { kind: "pending", name: "Take", message: null, armed: false };

test("the happy path runs idle to saved", () => {
  const t = run([{ type: "begin" }, { type: "started" }, { type: "stop" }, { type: "stopped", name: "Take" }, { type: "save" }, { type: "saved" }]);
  assert.deepEqual(t, { kind: "saved" });
});

test("a failed start returns to idle", () => {
  assert.deepEqual(run([{ type: "begin" }, { type: "startFailed" }]), idle);
});

test("a take can start again after saved or discarded, never while live", () => {
  assert.equal(step({ kind: "saved" }, { type: "begin" }).kind, "starting");
  assert.equal(step({ kind: "discarded" }, { type: "begin" }).kind, "starting");
  for (const live of [{ kind: "recording" }, { kind: "stopping" }, pending, { kind: "saving", name: "x" }] as Take[]) {
    assert.equal(step(live, { type: "begin" }), live);
  }
});

test("an interrupt while recording lands in pending with the reason and keeps the take", () => {
  const t = run([{ type: "interrupted", reason: "Microphone unplugged", name: "Take" }], { kind: "recording" });
  assert.deepEqual(t, { kind: "pending", name: "Take", message: "Microphone unplugged", armed: false });
  assert.equal(isLive(t), true);
});

test("an interrupt during stopping still lands in pending", () => {
  const t = step({ kind: "stopping" }, { type: "interrupted", reason: "Upload failed", name: "Take" });
  assert.equal(t.kind, "pending");
});

test("a late interrupt or stop changes nothing once the take is pending or saving", () => {
  const saving: Take = { kind: "saving", name: "x" };
  assert.equal(step(pending, { type: "interrupted", reason: "late", name: "y" }), pending);
  assert.equal(step(saving, { type: "interrupted", reason: "late", name: "y" }), saving);
  assert.equal(step(pending, { type: "stopped", name: "y" }), pending);
  assert.equal(step(idle, { type: "stop" }), idle);
});

test("a failed save returns to pending with the name and the message, ready to retry", () => {
  const t = run([{ type: "rename", name: "Elder, story 1" }, { type: "save" }, { type: "saveFailed", message: "disk full" }], pending);
  assert.deepEqual(t, { kind: "pending", name: "Elder, story 1", message: "disk full", armed: false });
  assert.equal(step(t, { type: "save" }).kind, "saving");
});

test("discard is deleted only after arm, from pending only", () => {
  assert.equal(step(pending, { type: "discard" }), pending, "one click is not enough");
  assert.equal(run([{ type: "arm" }, { type: "discard" }], pending).kind, "discarded");
  assert.equal(run([{ type: "arm" }, { type: "disarm" }, { type: "discard" }], pending).kind, "pending");
  for (const t of [idle, { kind: "recording" }, { kind: "stopping" }, { kind: "saving", name: "x" }] as Take[]) {
    assert.equal(step(step(t, { type: "arm" }), { type: "discard" }), t, `${t.kind} cannot be discarded`);
  }
});

test("a failed save disarms a discard that was armed", () => {
  const t = run([{ type: "save" }, { type: "saveFailed", message: "x" }], { ...pending, armed: true });
  assert.equal(t.kind === "pending" && t.armed, false);
});

test("only recording, stopping, pending, saving and starting block closing and project changes", () => {
  const live: Take[] = [{ kind: "starting" }, { kind: "recording" }, { kind: "stopping" }, pending, { kind: "saving", name: "x" }];
  const free: Take[] = [idle, { kind: "saved" }, { kind: "discarded" }];
  assert.deepEqual(live.map(isLive), live.map(() => true));
  assert.deepEqual(free.map(isLive), free.map(() => false));
});

test("keep for later leaves a pending take without discarding it, and only from pending", () => {
  assert.deepEqual(step({ ...pending, armed: true }, { type: "keep" }), idle);
  for (const t of [{ kind: "recording" }, { kind: "stopping" }, { kind: "saving", name: "x" }] as Take[]) {
    assert.equal(step(t, { type: "keep" }), t);
  }
});
