// The visiting wug on screen (rules in ./wug-visit.ts): the top bar's wug
// slips away, the same wug rises over the top edge of the panel being worked
// in, its eye follows the pointer, it blinks, and it sinks back out of sight
// while the top bar's wug returns. Touched, it ducks at once and stays away.
// Each visit owns an AbortController: every listener and timer of a visit
// ends with it, so a visit cut short leaves nothing running.
import { isPlaying } from "./playback";
import { currentWug, wugSvg } from "./theme/wug";
import { gaze, PEEK_MS, peekSpot, SCARED_FOR_MS, visitDue, visitsOn, type VisitState } from "./wug-visit";

const SIZE = 52;
const CHECK_EVERY_MS = 15_000;

export type Visits = { visitNow(): void; away(): boolean; stop(): void };

export function wireVisits(opts: { logo: () => Element | null; avoid: () => Element[]; busy: () => boolean; onLayout: (run: () => void) => void }): Visits {
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
  const s: VisitState = {
    enabled: visitsOn(localStorage),
    reducedMotion: reduced.matches,
    startedAt: Date.now(),
    lastVisit: 0,
    scaredUntil: 0,
    lastKey: 0,
    lastPointer: 0,
    busy: false,
  };
  let pointer = { x: 0, y: 0 };
  let buttons = 0;
  let dragging = false;
  let current: AbortController | null = null;
  let leave: (scared: boolean) => void = () => {};

  window.addEventListener("keydown", () => {
    s.lastKey = Date.now();
    leave(false);
  }, true);
  window.addEventListener("pointermove", (e) => {
    s.lastPointer = Date.now();
    pointer = { x: e.clientX, y: e.clientY };
    buttons = e.buttons;
  }, { passive: true });
  window.addEventListener("pointerdown", (e) => (buttons = e.buttons), true);
  window.addEventListener("pointerup", (e) => (buttons = e.buttons), true);
  document.addEventListener("dragstart", () => (dragging = true), true);
  document.addEventListener("dragend", () => (dragging = false), true);
  document.addEventListener("drop", () => (dragging = false), true);
  // The layout changed under a visit: the wug would float over the wrong place.
  window.addEventListener("resize", () => leave(false));
  opts.onLayout(() => leave(false));

  const isBusy = () => dragging || buttons !== 0 || isPlaying() || !document.hasFocus() || document.visibilityState !== "visible" || opts.busy();

  const visit = () => {
    if (current !== null) return;
    const card = document.querySelector(".dv-groupview.dv-active-group > .dv-content-container");
    s.lastVisit = Date.now();
    if (card === null) return;
    const spot = peekSpot(card.getBoundingClientRect(), SIZE, opts.avoid().map((e) => e.getBoundingClientRect()));
    if (spot === null) return;

    const ctl = new AbortController();
    current = ctl;
    const { signal } = ctl;
    const later = (ms: number, run: () => void) => {
      const id = window.setTimeout(() => {
        if (!signal.aborted) run();
      }, ms);
      signal.addEventListener("abort", () => window.clearTimeout(id));
    };

    const body = document.createElement("span");
    body.className = "wug-peek-body";
    body.innerHTML = wugSvg(currentWug(), "");
    const box = document.createElement("div");
    box.className = "wug-peek";
    box.setAttribute("aria-hidden", "true");
    Object.assign(box.style, { left: `${spot.x}px`, top: `${spot.y - SIZE}px`, width: `${SIZE}px`, height: `${SIZE}px` });
    box.append(body);
    document.body.append(box);
    const svg = body.querySelector("svg");
    const eyes = [...body.querySelectorAll<SVGPathElement>('[data-part^="eye"]')];
    opts.logo()?.classList.add("wug-away");

    let eyeAt = { x: spot.x + SIZE / 2, y: spot.y - SIZE / 2 };
    const look = () => {
      const g = gaze(eyeAt, pointer, 22);
      // Face the pointer: the wug is drawn facing right.
      const facingLeft = pointer.x < spot.x + SIZE / 2 - 40;
      if (svg !== null) svg.style.transform = facingLeft ? "scaleX(-1)" : "";
      for (const e of eyes) e.setAttribute("transform", `translate(${facingLeft ? -g.dx : g.dx} ${g.dy})`);
    };

    // Ends this visit only: a handler of an old box can never end a newer one.
    const end = (scared: boolean) => {
      if (signal.aborted) return;
      ctl.abort();
      current = null;
      leave = () => {};
      if (scared) {
        s.scaredUntil = Date.now() + SCARED_FOR_MS;
        box.classList.add("scared");
      }
      box.classList.remove("up");
      box.classList.add("leaving");
      window.setTimeout(() => {
        box.remove();
        // A newer visit may have begun meanwhile: its corner wug stays away.
        if (current === null) document.querySelectorAll(".wug-away").forEach((e) => e.classList.remove("wug-away"));
      }, 320);
    };
    leave = end;
    box.addEventListener("pointerenter", () => end(true), { signal });
    box.addEventListener("pointerdown", () => end(true), { signal });

    requestAnimationFrame(() =>
      later(240, () => {
        box.classList.add("up");
        later(450, () => {
          const r = eyes[0]?.getBoundingClientRect();
          if (r !== undefined && r.width > 0) eyeAt = { x: r.left + r.width / 2, y: r.top + r.height / 2 };
          look();
          window.addEventListener("pointermove", () => requestAnimationFrame(look), { passive: true, signal });
        });
        later(2_300, () => {
          box.classList.add("blink");
          later(160, () => box.classList.remove("blink"));
        });
        later(PEEK_MS, () => end(false));
      }),
    );
  };

  window.setInterval(() => {
    s.enabled = visitsOn(localStorage);
    if (!s.enabled) leave(false);
    s.reducedMotion = reduced.matches;
    s.busy = isBusy();
    if (current === null && visitDue(Date.now(), s)) visit();
  }, CHECK_EVERY_MS);

  return {
    /** For Settings: a visit now, to show what the setting does, unless the person is busy. */
    visitNow: () => {
      if (!reduced.matches && !isBusy()) visit();
    },
    /** End a visit now (the setting was turned off). */
    stop: () => leave(false),
    /** Whether the top bar's wug should be out of its corner (a redraw keeps it so). */
    away: () => current !== null,
  };
}
