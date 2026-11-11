import assert from "node:assert/strict";
import { test } from "node:test";
import { checksEnabled, compareVersions, formatSize, fraction, idle, manualResult, noticeText, setChecksEnabled, step, type Available, type UpdateState } from "./logic.ts";

const update: Available = { version: "0.2.0", currentVersion: "0.1.0", notes: "Faster timeline.", sizeBytes: 42_300_000 };

test("versions compare numerically, with pre-releases before their release", () => {
  assert.equal(compareVersions("0.10.0", "0.9.9"), 1);
  assert.equal(compareVersions("v1.2.3", "1.2.3"), 0);
  assert.equal(compareVersions("1.2", "1.2.0"), 0);
  assert.equal(compareVersions("1.0.0-alpha", "1.0.0"), -1);
  assert.equal(compareVersions("1.0.0-alpha.2", "1.0.0-alpha.10"), -1);
  assert.equal(compareVersions("1.0.0-alpha", "1.0.0-alpha.1"), -1);
  assert.equal(compareVersions("1.0.0-beta", "1.0.0-alpha.9"), 1);
  assert.equal(compareVersions("1.0.0+build.5", "1.0.0"), 0);
});

test("a check offers only a newer version and never starts a download", () => {
  const checking = step(idle, { type: "check", manual: false });
  assert.deepEqual(checking, { kind: "checking", manual: false });
  const offered = step(checking, { type: "checked", update });
  assert.equal(offered.kind, "available");
  assert.deepEqual(step(checking, { type: "checked", update: { ...update, version: "0.1.0" } }), idle);
  assert.deepEqual(step(checking, { type: "checked", update: { ...update, version: "0.0.9" } }), idle);
  assert.deepEqual(step(checking, { type: "checked", update: null }), idle);
  assert.deepEqual(step(checking, { type: "check-failed" }), idle);
});

test("a second check leaves a download or an offer alone", () => {
  const offered = step(step(idle, { type: "check", manual: false }), { type: "checked", update });
  assert.equal(step(offered, { type: "check", manual: true }), offered);
  const downloading = step(offered, { type: "download" });
  assert.equal(step(downloading, { type: "check", manual: true }), downloading);
  const ready = step(downloading, { type: "downloaded", message: null });
  assert.equal(step(ready, { type: "check", manual: true }), ready);
  assert.equal(noticeText(ready), "wugs 0.2.0 is downloaded and checked.");
});

test("Later hides the offer; a later manual check can bring it back", () => {
  const offered: UpdateState = { kind: "available", update, showNotes: false, message: null };
  const later = step(offered, { type: "later" });
  assert.deepEqual(later, { kind: "later", version: "0.2.0" });
  assert.equal(noticeText(later), null);
  assert.equal(step(later, { type: "check", manual: true }).kind, "checking");
});

test("download, progress, install; a failure returns to the offer with why", () => {
  const offered: UpdateState = { kind: "available", update, showNotes: true, message: null };
  const notesOff = step(offered, { type: "notes" });
  assert.equal(notesOff.kind === "available" && notesOff.showNotes, false);
  let s = step(offered, { type: "download" });
  assert.deepEqual(s, { kind: "downloading", update, done: 0, total: 42_300_000 });
  s = step(s, { type: "progress", done: 21_150_000, total: null });
  assert.equal(fraction(s), 0.5);
  assert.equal(noticeText(s), "Downloading wugs 0.2.0: 21 MB of 42 MB");
  assert.equal(step(s, { type: "install" }), s, "nothing installs before the download finishes");
  const ready = step(s, { type: "downloaded", message: null });
  assert.deepEqual(ready, { kind: "ready", update, message: null });
  const installing = step(ready, { type: "install" });
  assert.equal(installing.kind, "installing");
  // A refused install keeps the downloaded bytes: back to ready, never to a second download.
  const refused = step(installing, { type: "failed", message: "A recording is in progress, stop it first" });
  assert.deepEqual(refused, { kind: "ready", update, message: "A recording is in progress, stop it first" });
  assert.equal(step(refused, { type: "download" }), refused);
  // A failed download goes back to the offer.
  assert.deepEqual(step(s, { type: "failed", message: "offline" }), { kind: "available", update, showNotes: false, message: "offline" });
  assert.equal(step(idle, { type: "failed", message: "x" }), idle);
});

test("the notice names the version, the size and the running version", () => {
  assert.equal(noticeText({ kind: "available", update, showNotes: false, message: null }), "wugs 0.2.0 is available (42 MB). You have 0.1.0.");
  assert.equal(noticeText({ kind: "available", update: { ...update, sizeBytes: null }, showNotes: false, message: null }), "wugs 0.2.0 is available. You have 0.1.0.");
  assert.equal(noticeText({ kind: "downloading", update, done: 0, total: null }), "Downloading wugs 0.2.0: 1 KB");
  assert.equal(noticeText(idle), null);
});

test("sizes read like a download page", () => {
  assert.equal(formatSize(640_000), "640 KB");
  assert.equal(formatSize(8_520_000), "8.5 MB");
  assert.equal(formatSize(9_000_000), "9 MB");
  assert.equal(formatSize(123_400_000), "123 MB");
});

test("only a check the person asked for reports nothing new or a failure", () => {
  assert.equal(manualResult(false, { kind: "failed", error: "offline" }), null);
  assert.equal(manualResult(false, { kind: "newest", version: "0.1.0" }), null);
  assert.equal(manualResult(true, { kind: "found" }), null);
  assert.equal(manualResult(true, { kind: "newest", version: "0.1.0" }), "wugs 0.1.0 is the newest version.");
  assert.match(manualResult(true, { kind: "failed", error: "offline" })!, /Online.*offline/);
});

test("checking at start is on by default and can be switched off per machine", () => {
  const map = new Map<string, string>();
  const store = { getItem: (k: string) => map.get(k) ?? null, setItem: (k: string, v: string) => void map.set(k, v) };
  assert.equal(checksEnabled(store), true);
  setChecksEnabled(store, false);
  assert.equal(checksEnabled(store), false);
  setChecksEnabled(store, true);
  assert.equal(checksEnabled(store), true);
});
