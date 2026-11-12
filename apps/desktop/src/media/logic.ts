// The media bin's cards (roadmap 66, docs/design/EDITOR.md): pure rules.

export type BinSource = { id: string; label: string; kind: string | null; access: string | null; hasMedia: boolean };
export type BinText = { id: string; title: string; recording: string | null };

export type Card = {
  id: string;
  label: string;
  access: string;
  /** The text on screen uses this recording. */
  inThisText: boolean;
  /** Titles of the texts linked to it, in the texts' order. */
  usedBy: string[];
};

/** Recordings with audio, as cards, in the order the project lists them. */
export function cards(sources: readonly BinSource[], texts: readonly BinText[], shownText: string | null): Card[] {
  return sources
    .filter((s) => s.kind === "audio" && s.hasMedia)
    .map((s) => ({
      id: s.id,
      label: s.label || "(unnamed recording)",
      access: s.access ?? "open",
      inThisText: shownText !== null && texts.some((t) => t.id === shownText && t.recording === s.id),
      usedBy: texts.filter((t) => t.recording === s.id).map((t) => t.title || t.id),
    }));
}

/** "0:04", "3:07", "1:02:03": how video editors print clip lengths. */
export function duration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

/** An SVG path of the waveform's envelope in a width × height box: the
 * maxima across the top half, the minima back along the bottom. Samples are
 * in −1..1; an empty slice draws a flat line. */
export function wavePath(mins: readonly number[], maxs: readonly number[], width: number, height: number): string {
  const n = Math.min(mins.length, maxs.length);
  const mid = height / 2;
  if (n === 0) return `M0 ${mid} H${width}`;
  const x = (i: number) => (n === 1 ? width / 2 : (i * width) / (n - 1));
  const y = (v: number) => mid - Math.max(-1, Math.min(1, v)) * mid;
  const r = (v: number) => Math.round(v * 10) / 10;
  let d = `M${r(x(0))} ${r(y(maxs[0]!))}`;
  for (let i = 1; i < n; i++) d += ` L${r(x(i))} ${r(y(maxs[i]!))}`;
  for (let i = n - 1; i >= 0; i--) d += ` L${r(x(i))} ${r(y(mins[i]!))}`;
  return `${d} Z`;
}

/** Peaks as the backend gives them (16-bit sample values) scaled so the
 * loudest reaches ±1: a quiet field recording still shows its shape. */
export function normalized(mins: readonly number[], maxs: readonly number[]): { mins: number[]; maxs: number[] } {
  const top = Math.max(1, ...mins.map(Math.abs), ...maxs.map(Math.abs));
  return { mins: mins.map((v) => v / top), maxs: maxs.map((v) => v / top) };
}

/** The drag type a card carries onto the Timeline. */
export const CARD_TYPE = "application/x-wugs-recording";

/** Whether a media bin card is being dragged right now. Only a drag that
 * starts inside the app sets this, so a web page that puts `CARD_TYPE` on a
 * link it drags in is still refused as a link. */
let dragging = false;
export const cardDrag = {
  start: () => {
    dragging = true;
  },
  end: () => {
    dragging = false;
  },
  active: () => dragging,
};
