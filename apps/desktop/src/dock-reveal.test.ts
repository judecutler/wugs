import assert from "node:assert/strict";
import { test } from "node:test";
import { sideIndexes, REACH_ABOVE, REACH_BELOW, revealIndex, setTabMode, tabMode } from "./dock-reveal.ts";

const left = { left: 0, right: 500, top: 100, bottom: 390 };
const right = { left: 510, right: 1000, top: 100, bottom: 390 };
const lower = { left: 0, right: 500, top: 400, bottom: 700 };

test("the pointer near a group's top edge reveals that group", () => {
  assert.equal(revealIndex(50, 105, [left, right, lower]), 0);
  assert.equal(revealIndex(50, 92, [left, right, lower]), 0);
  assert.equal(revealIndex(600, 95, [left, right, lower]), 1);
  assert.equal(revealIndex(50, 405, [left, right, lower]), 2);
});

test("deep inside a panel nothing is revealed", () => {
  assert.equal(revealIndex(50, 100 + REACH_BELOW + 1, [left]), null);
  assert.equal(revealIndex(50, 100 - REACH_ABOVE - 1, [left]), null);
});

test("between a group's bottom and the next group's top, the nearer top edge wins", () => {
  const upper = { left: 0, right: 500, top: 100, bottom: 110 };
  const below = { left: 0, right: 500, top: 112, bottom: 400 };
  assert.equal(revealIndex(50, 111, [upper, below]), 1);
  assert.equal(revealIndex(50, 104, [upper, below]), 0);
});

test("a panel's first row never reveals tabs, but risen tabs stay up anywhere in their group", () => {
  assert.equal(revealIndex(50, 100 + REACH_BELOW + 3, [left]), null);
  assert.equal(revealIndex(50, 100 + REACH_BELOW + 3, [left], 0), 0);
  assert.equal(revealIndex(50, 380, [left], 0), 0);
  assert.equal(revealIndex(50, 391, [left], 0), null);
  assert.equal(revealIndex(600, 200, [left, right], 0), null);
});

test("from a risen group, the next group's top edge still takes over", () => {
  const top = { left: 0, right: 500, top: 100, bottom: 398 };
  const bottom = { left: 0, right: 500, top: 400, bottom: 700 };
  assert.equal(revealIndex(50, 396, [top, bottom], 0), 1);
  assert.equal(revealIndex(50, 300, [top, bottom], 0), 0);
});

test("outside a group's width it is not revealed", () => {
  assert.equal(revealIndex(505, 100, [left, right]), null);
});

test("the tab mode is remembered and defaults to reveal-on-reach", () => {
  const m = new Map<string, string>();
  const store = { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) };
  assert.equal(tabMode(store), "reach");
  setTabMode(store, "always");
  assert.equal(tabMode(store), "always");
});

test("the leftmost and rightmost groups are sides; middle and full-width ones are not", () => {
  const groups = [
    { left: 0, right: 250 },
    { left: 260, right: 1100 },
    { left: 260, right: 640 },
    { left: 650, right: 1100 },
    { left: 1110, right: 1430 },
  ];
  assert.deepEqual(sideIndexes(groups), [0, 4]);
  assert.deepEqual(sideIndexes([{ left: 0, right: 1430 }, { left: 0, right: 1430 }]), []);
  assert.deepEqual(sideIndexes([]), []);
});
