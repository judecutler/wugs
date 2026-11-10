// All timeline math lives here, pure and unit-tested: the viewport
// (time window <-> pixels), zoom quantized to √2 steps, and the strip
// grid the peak cache is keyed by. No canvas, no IPC.

/** What the timeline shows: a time window over a recording. */
export type Viewport = {
  /** Time at the left edge, ms. */
  t0Ms: number;
  /** Zoom: pixels per second of audio. */
  pxPerSec: number;
  /** Length of the recording, ms. */
  durationMs: number;
  /** Width of the drawing area, CSS pixels. */
  widthPx: number;
};

export const MAX_PX_PER_SEC = 10_000;
/** Peak data per strip, in pixels at the strip's bucket zoom. */
export const STRIP_PX = 2048;

export const timeToX = (v: Viewport, tMs: number): number => ((tMs - v.t0Ms) / 1000) * v.pxPerSec;
export const xToTime = (v: Viewport, x: number): number => v.t0Ms + (x / v.pxPerSec) * 1000;
export const visibleSpanMs = (v: Viewport): number => (v.widthPx / v.pxPerSec) * 1000;

/** The zoom that fits the whole recording; the floor for zooming out. */
export function fitPxPerSec(v: Viewport): number {
  if (v.durationMs <= 0 || v.widthPx <= 0) return 100;
  return Math.min(MAX_PX_PER_SEC, (v.widthPx * 1000) / v.durationMs);
}

/** Legal viewport nearest to the given one: zoom within [fit, max], the
 * window inside the recording (or at 0 when it all fits). */
export function clamp(v: Viewport): Viewport {
  const pxPerSec = Math.min(MAX_PX_PER_SEC, Math.max(fitPxPerSec(v), v.pxPerSec));
  const clamped = { ...v, pxPerSec };
  const maxT0 = Math.max(0, v.durationMs - visibleSpanMs(clamped));
  return { ...clamped, t0Ms: Math.min(maxT0, Math.max(0, v.t0Ms)) };
}

/** Zoom by `factor`, keeping the time under screen x where it is. */
export function zoomAround(v: Viewport, x: number, factor: number): Viewport {
  const anchor = xToTime(v, x);
  const pxPerSec = Math.min(MAX_PX_PER_SEC, Math.max(fitPxPerSec(v), v.pxPerSec * factor));
  return clamp({ ...v, pxPerSec, t0Ms: anchor - (x / pxPerSec) * 1000 });
}

export function pan(v: Viewport, dxPx: number): Viewport {
  return clamp({ ...v, t0Ms: v.t0Ms + (dxPx / v.pxPerSec) * 1000 });
}

// ------------------------------------------------------------------ strips
//
// Peak data is fetched in strips: STRIP_PX pixels of min/max pairs at a
// quantized zoom. Quantizing pxPerSec to powers of √2 keeps nearby zooms
// on one bucket, so strips survive a zoom gesture and scale at most ±19%
// when drawn at an off-bucket zoom.

export const zoomBucket = (pxPerSec: number): number => Math.round(Math.log2(pxPerSec) * 2);
export const bucketPxPerSec = (bucket: number): number => 2 ** (bucket / 2);

export const stripSpanMs = (bucket: number): number => (STRIP_PX / bucketPxPerSec(bucket)) * 1000;
// The epsilon absorbs float noise at exact strip boundaries
// (3 * span / span can land just under 3).
export const stripIndex = (bucket: number, tMs: number): number => Math.floor(tMs / stripSpanMs(bucket) + 1e-9);
export const stripStartMs = (bucket: number, index: number): number => index * stripSpanMs(bucket);

export const stripKey = (bucket: number, index: number, channel: number): string => `${bucket}:${index}:${channel}`;

/** Strips needed to draw `v` plus one strip of lookahead each side. */
export function stripRange(v: Viewport): { bucket: number; first: number; last: number } {
  const bucket = zoomBucket(v.pxPerSec);
  const lastVisible = Math.max(0, Math.min(v.durationMs, v.t0Ms + visibleSpanMs(v)) - 1e-9);
  return {
    bucket,
    first: Math.max(0, stripIndex(bucket, v.t0Ms) - 1),
    last: stripIndex(bucket, lastVisible) + 1,
  };
}

export const msToFrame = (tMs: number, sampleRate: number): number => Math.round((tMs / 1000) * sampleRate);

/** The peaks_query arguments for one strip. */
export function stripQuery(
  bucket: number,
  index: number,
  sampleRate: number,
): { startFrame: number; endFrame: number; pixels: number } {
  return {
    startFrame: msToFrame(stripStartMs(bucket, index), sampleRate),
    endFrame: msToFrame(stripStartMs(bucket, index + 1), sampleRate),
    pixels: STRIP_PX,
  };
}

/** One peaks_query answer. Self-describing in frames, so a strip fetched
 * for another zoom bucket still samples correctly, just coarser or finer. */
export type PeakStrip = {
  firstFrame: number;
  bucketFrames: number;
  mins: ArrayLike<number>;
  maxs: ArrayLike<number>;
};

export type MinMax = { min: number; max: number };

/** Min and max over the samples of `strip` covering [f0, f1) frames,
 * written into `out`; false when the strip holds none of them. The out
 * parameter keeps the draw loop free of per-pixel allocation. */
export function stripMinMaxInto(strip: PeakStrip, f0: number, f1: number, out: MinMax): boolean {
  const len = strip.mins.length;
  let lo = Math.floor((f0 - strip.firstFrame) / strip.bucketFrames);
  let hi = Math.ceil((f1 - strip.firstFrame) / strip.bucketFrames);
  if (hi <= 0 || lo >= len || f1 <= f0) return false;
  lo = Math.max(0, lo);
  // At least one sample, so a sub-sample range inside the strip answers.
  hi = Math.min(len, Math.max(hi, lo + 1));
  let min = Number.POSITIVE_INFINITY;
  let max = Number.NEGATIVE_INFINITY;
  for (let i = lo; i < hi; i++) {
    const sampleMin = strip.mins[i];
    const sampleMax = strip.maxs[i];
    if (sampleMin !== undefined && sampleMin < min) min = sampleMin;
    if (sampleMax !== undefined && sampleMax > max) max = sampleMax;
  }
  if (!Number.isFinite(min) || !Number.isFinite(max)) return false;
  out.min = min;
  out.max = max;
  return true;
}

export function stripMinMax(strip: PeakStrip, f0: number, f1: number): MinMax | null {
  const out = { min: 0, max: 0 };
  return stripMinMaxInto(strip, f0, f1, out) ? out : null;
}

/** Wait after a failed strip fetch before asking for the same strip again. */
export const STRIP_RETRY_MS = 2000;

/** Whether a strip that last failed at `failedAtMs` (undefined: never) may be requested at `nowMs`. */
export const mayRetryStrip = (failedAtMs: number | undefined, nowMs: number): boolean =>
  failedAtMs === undefined || nowMs - failedAtMs >= STRIP_RETRY_MS;
