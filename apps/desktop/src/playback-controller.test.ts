import assert from "node:assert/strict";
import { test } from "node:test";
import { createPlayback, type MediaLike, type Scheduler } from "./playback-controller.ts";

/** A media element that plays on demand: pause() queues its event, as browsers do. */
function fakeMedia() {
  const listeners: (() => void)[] = [];
  const pending: { resolve: () => void; reject: (e: unknown) => void }[] = [];
  const m = {
    currentTime: 0,
    paused: true,
    pauseCount: 0,
    play() {
      m.paused = false;
      return new Promise<void>((resolve, reject) => pending.push({ resolve, reject }));
    },
    pause() {
      for (const p of pending.splice(0)) p.reject(Object.assign(new Error("interrupted by pause"), { name: "AbortError" }));
      if (m.paused) return;
      m.paused = true;
      m.pauseCount++;
      queueMicrotask(() => listeners.forEach((l) => l()));
    },
    addEventListener(_t: "pause", l: () => void) {
      listeners.push(l);
    },
  } satisfies MediaLike & { pauseCount: number };
  return { m, pending };
}

function fakeSched() {
  let id = 0;
  const frames = new Map<number, () => void>();
  const timers = new Map<number, { at: number; cb: () => void }>();
  let now = 0;
  const s: Scheduler = {
    raf: (cb) => (frames.set(++id, cb), id),
    cancelRaf: (i) => void frames.delete(i),
    setTimeout: (cb, ms) => (timers.set(++id, { at: now + ms, cb }), id),
    clearTimeout: (i) => void timers.delete(i),
  };
  return {
    s,
    frame() {
      const all = [...frames.values()];
      frames.clear();
      all.forEach((f) => f());
    },
    advance(ms: number) {
      now += ms;
      for (const [i, t] of [...timers]) {
        if (t.at <= now) {
          timers.delete(i);
          t.cb();
        }
      }
    },
  };
}

const tick = () => new Promise<void>((r) => setImmediate(r));
const setup = () => {
  const { m, pending } = fakeMedia();
  const sched = fakeSched();
  const errors: unknown[] = [];
  const pb = createPlayback(m, sched.s, (e) => errors.push(e));
  return { m, pending, sched, errors, pb };
};
const abort = () => Object.assign(new Error("interrupted"), { name: "AbortError" });

test("a range stops at its end", async () => {
  const { m, pending, sched, pb } = setup();
  pb.playRange(1000, 2000);
  pending[0]?.resolve();
  await tick();
  m.currentTime = 1.5;
  sched.frame();
  assert.equal(m.paused, false);
  m.currentTime = 2.01;
  sched.frame();
  assert.equal(m.paused, true);
});

test("B2: a range started while another plays keeps its own end", async () => {
  const { m, pending, sched, pb } = setup();
  pb.playRange(0, 1000);
  pending.shift()?.resolve();
  await tick();
  pb.playRange(5000, 6000);
  await tick();
  pending.shift()?.resolve();
  await tick();
  m.currentTime = 5.5;
  sched.frame();
  assert.equal(m.paused, false);
  m.currentTime = 6.01;
  sched.frame();
  assert.equal(m.paused, true);
});

test("B2: two rapid ranges, the first play() aborted, stop at the second end without a toast", async () => {
  const { m, pending, sched, errors, pb } = setup();
  pb.playRange(0, 1000);
  pb.playRange(5000, 6000);
  await tick();
  pending.shift()?.resolve();
  await tick();
  assert.deepEqual(errors, []);
  m.currentTime = 5.5;
  sched.frame();
  assert.equal(m.paused, false);
  m.currentTime = 6.01;
  sched.frame();
  assert.equal(m.paused, true);
});

test("B2: an AbortError neither toasts nor clears the stop", async () => {
  const { m, pending, sched, errors, pb } = setup();
  pb.playRange(1000, 2000);
  pending.shift()?.reject(abort());
  await tick();
  assert.deepEqual(errors, []);
  m.currentTime = 2.5;
  sched.frame();
  assert.equal(m.paused, true);
});

test("a play() that fails for real toasts and drops the stop", async () => {
  const { m, pending, sched, errors, pb } = setup();
  pb.playRange(1000, 2000);
  pending.shift()?.reject(new Error("NotAllowedError"));
  await tick();
  assert.equal(errors.length, 1);
  m.paused = false;
  m.currentTime = 9;
  sched.frame();
  assert.equal(m.paused, false);
});

test("M1: with animation frames stopped (hidden window), a timer still ends the range", async () => {
  const { m, pending, sched, pb } = setup();
  pb.playRange(1000, 3000);
  pending.shift()?.resolve();
  await tick();
  m.currentTime = 1.0;
  sched.advance(1500);
  assert.equal(m.paused, false);
  m.currentTime = 3.2;
  sched.advance(1000);
  assert.equal(m.paused, true);
});

test("M1: a timer that fires before the end re-arms for the rest", async () => {
  const { m, pending, sched, pb } = setup();
  pb.playRange(0, 2000);
  pending.shift()?.resolve();
  await tick();
  m.currentTime = 0.5;
  sched.advance(2000);
  assert.equal(m.paused, false);
  m.currentTime = 2.0;
  sched.advance(1500);
  assert.equal(m.paused, true);
});

test("M2: seeking during a range drops its stop", async () => {
  const { m, pending, sched, pb } = setup();
  pb.playRange(1000, 2000);
  pending.shift()?.resolve();
  await tick();
  pb.seek(30000);
  sched.frame();
  sched.advance(60000);
  assert.equal(m.paused, false);
});

test("M2: toggling drops the stop, so resuming plays on", async () => {
  const { m, pending, sched, pb } = setup();
  pb.playRange(1000, 2000);
  pending.shift()?.resolve();
  await tick();
  pb.toggle();
  await tick();
  pb.toggle();
  pending.shift()?.resolve();
  await tick();
  m.currentTime = 9;
  sched.frame();
  sched.advance(60000);
  assert.equal(m.paused, false);
});
