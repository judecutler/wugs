// Praat analysis overlays (roadmap item 46): which chunks a view needs and
// where values sit on screen. Pure, so node --test runs them. Chunks come
// from the acoustic_analysis command (src-tauri/src/praat_analysis.rs).

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

/** A recording's voice settings (src-tauri/src/praat_analysis.rs `Voice`). */
export type Voice = { id: string; name: string; pitchFloor: number; pitchCeiling: number; formantCeiling: number };

/** The ranges drawn for a voice: its pitch range and formant ceiling. */
export function rangesFor(v: Voice | null): Record<Kind, { lo: number; hi: number; unit: string }> {
  if (v === null) return RANGES;
  return { pitch: { lo: v.pitchFloor, hi: v.pitchCeiling, unit: "Hz" }, intensity: RANGES.intensity, formants: { lo: 0, hi: v.formantCeiling, unit: "Hz" } };
}

/** Height of a value in a band from `top` to `top + height`; null outside the range. */
export function yFor(kind: Kind, value: number, top: number, height: number, ranges = RANGES): number | null {
  const { lo, hi } = ranges[kind];
  if (value < lo || value > hi) return null;
  return top + height - ((value - lo) / (hi - lo)) * height;
}

/** Time of frame i of a chunk, ms (frames sit mid-step). */
export function frameMs(chunk: Chunk, i: number): number {
  return chunk.startMs + (i + 0.5) * chunk.stepMs;
}

/** What the analysis says at one moment: the nearest frame within one step. */
export type Reading = { ms: number; pitch: number | null; intensity: number | null; f1: number | null; f2: number | null; f3: number | null };

export function readAt(chunks: Iterable<Chunk>, ms: number): Reading | null {
  for (const c of chunks) {
    const n = c.pitch.length;
    if (n === 0 || ms < c.startMs || ms >= c.startMs + n * c.stepMs) continue;
    const i = Math.min(n - 1, Math.max(0, Math.round((ms - c.startMs) / c.stepMs - 0.5)));
    const at = (xs: (number | null)[]) => xs[i] ?? null;
    return { ms: frameMs(c, i), pitch: at(c.pitch), intensity: at(c.intensity), f1: at(c.f1), f2: at(c.f2), f3: at(c.f3) };
  }
  return null;
}

/** The reading in words, for the kinds shown: «212 Hz · 68 dB · F1 540 F2 1720 F3 2610», with the time. */
export function readingText(r: Reading, on: ReadonlySet<Kind>): string {
  const hz = (v: number | null) => (v === null ? "–" : String(Math.round(v)));
  const parts = [`${(r.ms / 1000).toFixed(3)} s`];
  if (on.has("pitch")) parts.push(r.pitch === null ? "unvoiced" : `${hz(r.pitch)} Hz`);
  if (on.has("intensity")) parts.push(r.intensity === null ? "– dB" : `${r.intensity.toFixed(1)} dB`);
  if (on.has("formants")) parts.push(`F1 ${hz(r.f1)} F2 ${hz(r.f2)} F3 ${hz(r.f3)}`);
  return parts.join(" · ");
}
