// The window's one audio element. The timeline loads the recording into
// it; any panel can play a range of it. Never create a second element.
import { errorText } from "./api";
import { createPlayback } from "./playback-controller";
import { showToast } from "./store";

const audio = new Audio();
audio.preload = "auto";

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
}

export const hasSource = (): boolean => audio.hasAttribute("src");
export const currentMs = (): number => audio.currentTime * 1000;
export const isPlaying = (): boolean => !audio.paused;
export const pause = (): void => ctl.pause();
export const seek = (tMs: number): void => ctl.seek(tMs);
/** The recording track's mute: silence without stopping the playhead. */
export const setMuted = (on: boolean): void => {
  audio.muted = on;
};

export function toggle(): void {
  if (hasSource()) ctl.toggle();
}

/** Play from startMs and stop at endMs. A no-op without a recording. */
export function playRange(startMs: number, endMs: number): void {
  if (hasSource()) ctl.playRange(startMs, endMs);
}
