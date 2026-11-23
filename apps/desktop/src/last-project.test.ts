import assert from "node:assert/strict";
import { test } from "node:test";
import { autoOpen, forgetProject, lastProject, recentProjects, rememberProject } from "./last-project.ts";

const memory = () => {
  const m = new Map<string, string>();
  return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) };
};
const describe = (e: unknown) => String(e);
const closed = () => false;

test("with nothing remembered, nothing is opened", async () => {
  let calls = 0;
  const r = await autoOpen(memory(), "prs_A", closed, async () => void calls++, describe);
  assert.deepEqual(r, { kind: "skipped" });
  assert.equal(calls, 0);
});

test("a remembered project is opened once, with the researcher", async () => {
  const store = memory();
  rememberProject(store, "/field/lezgi");
  const seen: string[] = [];
  const r = await autoOpen(store, "prs_A", closed, async (p, who) => void seen.push(p, who), describe);
  assert.deepEqual(r, { kind: "opened" });
  assert.deepEqual(seen, ["/field/lezgi", "prs_A"]);
});

test("a failure is reported and keeps the path for the next launch", async () => {
  const store = memory();
  rememberProject(store, "/usb/lezgi");
  const r = await autoOpen(store, "prs_A", closed, async () => Promise.reject("no wugs project at /usb/lezgi"), describe);
  assert.deepEqual(r, { kind: "failed", path: "/usb/lezgi", error: "no wugs project at /usb/lezgi" });
  assert.equal(lastProject(store), "/usb/lezgi");
});

test("without a researcher id the remembered project is left alone", async () => {
  const store = memory();
  rememberProject(store, "/field/lezgi");
  let calls = 0;
  assert.deepEqual(await autoOpen(store, "  ", closed, async () => void calls++, describe), { kind: "skipped" });
  assert.equal(calls, 0);
});

test("a project the person already opened is never replaced", async () => {
  const store = memory();
  rememberProject(store, "/field/lezgi");
  let calls = 0;
  assert.deepEqual(await autoOpen(store, "prs_A", () => true, async () => void calls++, describe), { kind: "skipped" });
  assert.equal(calls, 0);
});

test("recent project folders: the latest first, each once, at most six", () => {
  const store = memory();
  assert.deepEqual(recentProjects(store), []);
  store.setItem("wugs.lastProject", "/old");
  assert.deepEqual(recentProjects(store), ["/old"], "an older version's last project is kept");
  for (const p of ["/a", "/b", "/a", "/c", "/d", "/e", "/f", "/g"]) rememberProject(store, p);
  assert.deepEqual(recentProjects(store), ["/g", "/f", "/e", "/d", "/c", "/a"]);
  forgetProject(store, "/g");
  assert.deepEqual(recentProjects(store), ["/f", "/e", "/d", "/c", "/a"]);
  assert.equal(lastProject(store), "", "the forgotten one is not reopened at start");
  store.setItem("wugs.recentProjects", "not json");
  assert.deepEqual(recentProjects(store), []);
});
