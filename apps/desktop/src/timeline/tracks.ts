// Track headers (docs/design/EDITOR.md): lock and hide on every tier track,
// mute on the recording. How one person views one text on one computer, like
// a video editor's track toggles: kept per text in local storage, never in
// the op log, so they never sync and never conflict.

export type Tracks = { hidden: string[]; locked: string[]; muted: boolean };

export const NO_TRACKS: Tracks = { hidden: [], locked: [], muted: false };

export const tracksKey = (textId: string): string => `wugs.timeline.tracks.${textId}`;

/** Parse what local storage holds; anything unexpected reads as no toggles. */
export function parseTracks(raw: string | null): Tracks {
  if (raw === null) return NO_TRACKS;
  try {
    const v = JSON.parse(raw) as Partial<Tracks>;
    const ids = (x: unknown): string[] => (Array.isArray(x) ? x.filter((s): s is string => typeof s === "string") : []);
    return { hidden: ids(v.hidden), locked: ids(v.locked), muted: v.muted === true };
  } catch {
    return NO_TRACKS;
  }
}

const flip = (list: string[], id: string): string[] => (list.includes(id) ? list.filter((x) => x !== id) : [...list, id]);

export const toggleHidden = (t: Tracks, tierId: string): Tracks => ({ ...t, hidden: flip(t.hidden, tierId) });
export const toggleLocked = (t: Tracks, tierId: string): Tracks => ({ ...t, locked: flip(t.locked, tierId) });
export const toggleMuted = (t: Tracks): Tracks => ({ ...t, muted: !t.muted });

/** Whether the timeline may change segments on this track, and why not. */
export function editBlocker(t: Tracks, tierId: string, name: string): string | null {
  if (t.hidden.includes(tierId)) return `The ${name} track is hidden. Show it to edit it.`;
  if (t.locked.includes(tierId)) return `The ${name} track is locked. Unlock it to change its segments.`;
  return null;
}

/** J, K, L as video editors use them: back, stop, forward. */
export type Shuttle = { kind: "seek"; byMs: number } | { kind: "pause" } | { kind: "play" };
export function shuttle(key: string): Shuttle | null {
  switch (key) {
    case "j":
    case "J":
      return { kind: "seek", byMs: -2000 };
    case "k":
    case "K":
      return { kind: "pause" };
    case "l":
    case "L":
      return { kind: "play" };
    default:
      return null;
  }
}

/** "1:02.3" for the transport row; hours when a recording is that long. */
export function clock(ms: number): string {
  const t = Math.max(0, ms) / 1000;
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const s = t % 60;
  const ss = s.toFixed(1).padStart(4, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

/** The zoom slider runs on a log scale between the view's fit and the
 * closest zoom, 0 to 1000, so every step feels the same size. */
export function sliderToPxPerSec(value: number, fitPx: number, maxPx: number): number {
  const lo = Math.log(fitPx);
  const hi = Math.log(Math.max(maxPx, fitPx * 1.0001));
  return Math.exp(lo + (Math.min(1000, Math.max(0, value)) / 1000) * (hi - lo));
}

export function pxPerSecToSlider(px: number, fitPx: number, maxPx: number): number {
  const lo = Math.log(fitPx);
  const hi = Math.log(Math.max(maxPx, fitPx * 1.0001));
  return Math.round(Math.min(1, Math.max(0, (Math.log(Math.max(px, 1e-6)) - lo) / (hi - lo))) * 1000);
}
