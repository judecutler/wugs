// Range playback over one media element, apart from the DOM: the element and
// the scheduler are injected so the stop logic runs under test.
export type MediaLike = {
  currentTime: number;
  readonly paused: boolean;
  play(): Promise<void>;
  pause(): void;
};

export type Scheduler = {
  raf(cb: () => void): number;
  cancelRaf(id: number): void;
  setTimeout(cb: () => void, ms: number): number;
  clearTimeout(id: number): void;
};

const isAbort = (e: unknown) => e instanceof Error && e.name === "AbortError";

export function createPlayback(media: MediaLike, sched: Scheduler, onError: (e: unknown) => void) {
  let stopAtMs: number | null = null;
  /** A looped range: played again after `gapMs` each time it reaches its end. */
  let loop: { startMs: number; endMs: number; gapMs: number } | null = null;
  /** Each range gets a number; a stale one's callbacks do nothing, so a newer range keeps its stop. */
  let generation = 0;
  let frame = 0;
  let timer = 0;

  function clearStop(): void {
    generation += 1;
    stopAtMs = null;
    loop = null;
    sched.cancelRaf(frame);
    sched.clearTimeout(timer);
  }

  /** Stop at the end, or give up when something else paused the element. */
  function check(gen: number, viaTimer: boolean): void {
    if (gen !== generation || stopAtMs === null) return;
    const remaining = stopAtMs - media.currentTime * 1000;
    if (remaining <= 0 || media.paused) {
      // Only a range that reached its end loops; a pause from elsewhere ends it.
      const again = remaining <= 0 && !media.paused ? loop : null;
      if (!media.paused) media.pause();
      clearStop();
      if (again !== null) {
        const g = generation;
        timer = sched.setTimeout(() => {
          if (g === generation) start(again.startMs, again.endMs, again.gapMs);
        }, again.gapMs);
      }
    } else if (viaTimer) {
      timer = sched.setTimeout(() => check(gen, true), remaining);
    }
  }

  function watch(gen: number): void {
    const step = () => {
      check(gen, false);
      if (gen === generation) frame = sched.raf(step);
    };
    frame = sched.raf(step);
    // Animation frames stop while the window is hidden; a timer still ends the range.
    timer = sched.setTimeout(() => check(gen, true), Math.max(0, (stopAtMs ?? 0) - media.currentTime * 1000));
  }

  const seek = (tMs: number) => {
    clearStop();
    media.currentTime = tMs / 1000;
  };

  function start(startMs: number, endMs: number, loopGapMs: number | null): void {
    clearStop();
    const gen = generation;
    media.pause();
    media.currentTime = startMs / 1000;
    stopAtMs = endMs;
    loop = loopGapMs === null ? null : { startMs, endMs, gapMs: loopGapMs };
    media.play().catch((e) => {
      if (isAbort(e)) return;
      if (gen === generation) clearStop();
      onError(e);
    });
    watch(gen);
  }

  return {
    seek,
    pause: () => media.pause(),
    toggle(): void {
      clearStop();
      if (media.paused) media.play().catch((e) => isAbort(e) || onError(e));
      else media.pause();
    },
    /** Play from startMs and stop at endMs; with a gap, play it again after
     * that many ms each time it ends, until anything else plays, seeks or pauses. */
    playRange(startMs: number, endMs: number, loopGapMs: number | null = null): void {
      start(startMs, endMs, loopGapMs);
    },
    /** Drop any range stop, for a source change. */
    reset(): void {
      media.pause();
      clearStop();
    },
  };
}
