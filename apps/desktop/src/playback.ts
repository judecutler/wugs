// The window's one audio element. The timeline loads the recording into
// it; any panel can play a range of it. Never create a second element.
import { errorText } from "./api";
import { createPlayback } from "./playback-controller";
import { showToast } from "./store";
import { RATE_EVENT, RATE_KEY, RATES, parseRate, rateLabel } from "./playback-rate";
import { h } from "./dom";

const audio = new Audio();
audio.preload = "auto";
/** Slowed or sped speech keeps its pitch. */
audio.preservesPitch = true;
let rate = parseRate(localStorage.getItem(RATE_KEY));
audio.defaultPlaybackRate = rate;
audio.playbackRate = rate;

/** The speed every recording plays at in this window. */
export const playbackRate = (): number => rate;

export function setPlaybackRate(r: number): void {
  rate = r;
  // Loading a source resets playbackRate to the default, so set both.
  audio.defaultPlaybackRate = r;
  audio.playbackRate = r;
  localStorage.setItem(RATE_KEY, String(r));
  window.dispatchEvent(new CustomEvent(RATE_EVENT));
}

/** A speed menu that follows every other one in the window. */
export function rateSelect(): HTMLSelectElement {
  const el = h("select", { className: "rate-select", title: "How fast recordings play here. Slowed speech keeps its pitch." }, ...RATES.map((r) => h("option", { value: String(r), textContent: rateLabel(r) })));
  el.setAttribute("aria-label", "Playback speed");
  el.value = String(rate);
  el.onchange = () => {
    setPlaybackRate(Number(el.value));
    el.blur();
  };
  // Held weakly, so a closed panel's menu is not kept alive by the window.
  const ref = new WeakRef(el);
  const sync = () => {
    const menu = ref.deref();
    if (menu === undefined) window.removeEventListener(RATE_EVENT, sync);
    else menu.value = String(rate);
  };
  window.addEventListener(RATE_EVENT, sync);
  return el;
}

const ctl = createPlayback(
  audio,
  {
    raf: (cb) => requestAnimationFrame(cb),
    cancelRaf: (id) => cancelAnimationFrame(id),
    setTimeout: (cb, ms) => window.setTimeout(cb, ms),
    clearTimeout: (id) => window.clearTimeout(id),
  },
  (e) => showToast({ tone: "error", lines: [`Playback failed: ${errorText(e)}`] }),
);

/** Point the element at a recording, or at nothing. */
export function setSource(url: string | null): void {
  ctl.reset();
  // src = "" would resolve to the page URL, so clear by attribute.
  if (url !== null) audio.src = url;
  else audio.removeAttribute("src");
  audio.playbackRate = rate;
}

export const hasSource = (): boolean => audio.hasAttribute("src");
/** The URL loaded, as set: so a second load of the same recording can be skipped. */
export const sourceUrl = (): string | null => audio.getAttribute("src");
export const currentMs = (): number => audio.currentTime * 1000;
export const isPlaying = (): boolean => !audio.paused;
export const pause = (): void => ctl.pause();
export const seek = (tMs: number): void => ctl.seek(tMs);
/** The Timeline's recording-track mute (docs/design/EDITOR.md): it
 * silences the Timeline's own playing (Space, the play button, J/K/L), not a
 * range another panel plays, such as Transcribe's replay. */
let timelineMuted = false;
export const setMuted = (on: boolean): void => {
  timelineMuted = on;
  if (!audio.paused && !rangePlaying) audio.muted = on;
};
let rangePlaying = false;

export function toggle(): void {
  if (!hasSource()) return;
  rangePlaying = false;
  audio.muted = timelineMuted;
  ctl.toggle();
}

/** Play from startMs and stop at endMs. A no-op without a recording. */
export function playRange(startMs: number, endMs: number): void {
  if (!hasSource()) return;
  rangePlaying = true;
  audio.muted = false;
  ctl.playRange(startMs, endMs);
}
