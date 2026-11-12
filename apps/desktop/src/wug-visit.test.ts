import assert from "node:assert/strict";
import { test } from "node:test";
import { FIRST_VISIT_AFTER_MS, gaze, peekSpot, QUIET_AFTER_KEY_MS, SCARED_FOR_MS, setVisitsOn, VISIT_EVERY_MS, visitDue, visitsOn, type VisitState } from "./wug-visit.ts";

const now = 100 * 60_000;
const ready: VisitState = {
  enabled: true,
  reducedMotion: false,
  startedAt: now - FIRST_VISIT_AFTER_MS - 1,
  lastVisit: now - VISIT_EVERY_MS - 1,
  scaredUntil: 0,
  lastKey: now - QUIET_AFTER_KEY_MS - 1,
  lastPointer: now - 1_000,
  busy: false,
};

test("a visit is due when the person is around, not typing, and it has been a while", () => {
  assert.equal(visitDue(now, ready), true);
});

test("no visit while typing, busy, scared, too soon, away, turned off or with reduced motion", () => {
  assert.equal(visitDue(now, { ...ready, lastKey: now - 500 }), false);
  assert.equal(visitDue(now, { ...ready, busy: true }), false);
  assert.equal(visitDue(now, { ...ready, scaredUntil: now + SCARED_FOR_MS }), false);
  assert.equal(visitDue(now, { ...ready, lastVisit: now - 60_000 }), false);
  assert.equal(visitDue(now, { ...ready, startedAt: now - 10_000 }), false);
  assert.equal(visitDue(now, { ...ready, lastPointer: now - 10 * 60_000 }), false);
  assert.equal(visitDue(now, { ...ready, enabled: false }), false);
  assert.equal(visitDue(now, { ...ready, reducedMotion: true }), false);
});

test("the wug peeks near the right end of a card's top edge, only where it fits", () => {
  assert.deepEqual(peekSpot({ left: 100, top: 300, right: 700, bottom: 800 }, 48), { x: 700 - 48 - 28, y: 300 });
  assert.equal(peekSpot({ left: 100, top: 300, right: 250, bottom: 800 }, 48), null);
  assert.equal(peekSpot({ left: 100, top: 300, right: 700, bottom: 360 }, 48), null);
  assert.equal(peekSpot({ left: 100, top: 10, right: 700, bottom: 800 }, 48), null);
});

test("the wug never peeks over the top bar's controls: it moves left until it is clear", () => {
  const card = { left: 100, top: 300, right: 700, bottom: 800 };
  const menus = { left: 560, top: 270, right: 700, bottom: 290 };
  const spot = peekSpot(card, 48, [menus]);
  assert.ok(spot !== null && spot.x + 48 <= 560);
  assert.equal(peekSpot(card, 48, [{ left: 0, top: 260, right: 2000, bottom: 280 }]), null);
});

test("the eye looks toward the pointer, never further than its reach", () => {
  const g = gaze({ x: 0, y: 0 }, { x: 1000, y: 0 }, 6);
  assert.ok(Math.abs(g.dx - 6) < 1e-9 && g.dy === 0);
  const near = gaze({ x: 0, y: 0 }, { x: 0, y: 50 }, 6);
  assert.ok(near.dy > 0 && near.dy < 6);
  assert.deepEqual(gaze({ x: 5, y: 5 }, { x: 5, y: 5 }, 6), { dx: 0, dy: 0 });
});

test("visits are on unless turned off, and the choice is kept", () => {
  const m = new Map<string, string>();
  const store = { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) };
  assert.equal(visitsOn(store), true);
  setVisitsOn(store, false);
  assert.equal(visitsOn(store), false);
});
