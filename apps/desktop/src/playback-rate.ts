// How fast recordings play: one speed for the window's audio element, chosen
// from the Timeline's transport row or the Transcribe toolbar and kept on
// this computer. Slowed speech keeps its pitch (the browser's own
// time-stretch), so a transcriber hears fast speech word by word.

export const RATES = [0.5, 0.75, 1, 1.25, 1.5] as const;
export const RATE_KEY = "wugs.playback.rate";
/** Sent on `window` when the speed changes, so every speed menu shows it. */
export const RATE_EVENT = "wugs:playback-rate";

/** A stored speed read back: one of RATES, else normal speed. */
export function parseRate(stored: string | null): number {
  const n = Number(stored);
  return (RATES as readonly number[]).includes(n) ? n : 1;
}

/** What a speed menu shows: "1×", "0.75×". */
export const rateLabel = (r: number): string => (r === 1 ? "1× speed" : `${r}× speed`);
