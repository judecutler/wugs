import { test } from "node:test";
import assert from "node:assert/strict";
import { fileTutorial, tutorialFiled } from "./logic.ts";

const store = () => {
  const m = new Map<string, string>();
  return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) };
};

test("the button shows until the tutorial is first opened, then stays filed", () => {
  const s = store();
  assert.equal(tutorialFiled(s), false);
  fileTutorial(s);
  assert.equal(tutorialFiled(s), true);
  fileTutorial(s);
  assert.equal(tutorialFiled(s), true);
});

test("storage that refuses writes leaves the button in place", () => {
  const s = { getItem: () => null, setItem: () => { throw new Error("full"); } };
  fileTutorial(s);
  assert.equal(tutorialFiled(s), false);
});
