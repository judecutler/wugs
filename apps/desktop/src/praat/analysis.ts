// Praat analysis overlays (roadmap item 46): which chunks a view needs and
// where values sit on screen. Pure, so node --test runs them. Chunks come
// from the praat_analysis command (src-tauri/src/praat_analysis.rs).

export const CHUNK_MS = 10_000;
/** Wider views are not analysed: one request is at most six chunks. */
export const MAX_SPAN_MS = 60_000;

export type Kind = "pitch" | "intensity" | "formants";
export const KINDS: Kind[] = ["pitch", "intensity", "formants"];

export type Chunk = {
  index: number;
  startMs: number;
  stepMs: number;
  pitch: (number | null)[];
  intensity: (number | null)[];
  f1: (number | null)[];
  f2: (number | null)[];
  f3: (number | null)[];
};

/** The chunks covering startMs–endMs, or null when the view is too wide to analyse. */
export function chunksFor(startMs: number, endMs: number, durationMs: number): number[] | null {
  const end = Math.min(endMs, durationMs);
  const start = Math.max(0, startMs);
  if (end <= start) return [];
  if (end - start > MAX_SPAN_MS) return null;
  const first = Math.floor(start / CHUNK_MS);
  const last = Math.ceil(end / CHUNK_MS) - 1;
  const out: number[] = [];
  for (let i = first; i <= last; i++) out.push(i);
  return out;
}

/** Each kind's range, as Praat's editor shows it (pitch 75–600 Hz to match the analysis). */
export const RANGES: Record<Kind, { lo: number; hi: number; unit: string }> = {
  pitch: { lo: 75, hi: 600, unit: "Hz" },
  intensity: { lo: 50, hi: 100, unit: "dB" },
  formants: { lo: 0, hi: 5500, unit: "Hz" },
};

/** Height of a value in a band from `top` to `top + height`; null outside the range. */
export function yFor(kind: Kind, value: number, top: number, height: number): number | null {
  const { lo, hi } = RANGES[kind];
  if (value < lo || value > hi) return null;
  return top + height - ((value - lo) / (hi - lo)) * height;
}

/** Time of frame i of a chunk, ms (frames sit mid-step). */
export function frameMs(chunk: Chunk, i: number): number {
  return chunk.startMs + (i + 0.5) * chunk.stepMs;
}
