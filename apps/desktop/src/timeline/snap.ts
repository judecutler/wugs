// Edge snapping, pure and unit-tested: a segment edge drawn or dragged
// near where speech starts or ends is pulled outward onto that point, so a
// hand-drawn box does not cut off the start of a word ("th" of "three").
// Outward only: a start moves earlier, an end later, so snapping never cuts
// more speech than the person drew; a dragged edge snaps only when it moves
// outward from where it was, so trimming inward is never undone by a snap.
// Holding Alt turns it off (the view
// decides that); spans that are not ready yet mean no snapping, never a wait.

/** A stretch of speech, [startMs, endMs], in time order. */
export type Span = readonly [number, number];

/** How far an edge may jump to reach a speech edge, ms. */
export const SNAP_MS = 250;

/**
 * Where an edge at `ms` lands. Only an edge that falls inside a stretch of
 * speech moves: a start to that stretch's start, an end to its end, when
 * that is within `tolMs`. An edge in a pause stays, so a box drawn over one
 * word never grows over the word before or after it (word spans are short,
 * so "the nearest start within 250 ms" would often be the previous word's).
 * `bound` keeps the jump off a neighbouring segment. No candidate, no move.
 */
export function snapEdge(edge: "start" | "end", ms: number, spans: readonly Span[], tolMs = SNAP_MS, bound?: number): number {
  for (const [s, e] of spans) {
    if (!(ms > s && ms < e)) continue;
    const point = edge === "start" ? s : e;
    if (Math.abs(ms - point) > tolMs) return ms;
    if (bound !== undefined && (edge === "start" ? point < bound : point > bound)) return ms;
    return point;
  }
  return ms;
}

/**
 * Where a dragged edge lands. It snaps only while it is outward of where
 * the drag started (a start earlier, an end later), so trimming a segment
 * inward near a speech edge stays where the person put it instead of
 * jumping back out; Alt is not needed for that.
 */
export function snapDragged(edge: "start" | "end", ms: number, fromMs: number, spans: readonly Span[], bound?: number, tolMs = SNAP_MS): number {
  const outward = edge === "start" ? ms < fromMs : ms > fromMs;
  return outward ? snapEdge(edge, ms, spans, tolMs, bound) : ms;
}

/** Both edges of a drawn range snapped outward, each within its bound. */
export function snapRange(
  startMs: number,
  endMs: number,
  spans: readonly Span[],
  bounds: { lo?: number; hi?: number } = {},
  tolMs = SNAP_MS,
): { startMs: number; endMs: number } {
  return { startMs: snapEdge("start", startMs, spans, tolMs, bounds.lo), endMs: snapEdge("end", endMs, spans, tolMs, bounds.hi) };
}

/**
 * The room around `[startMs, endMs]` on a lane: the end of the nearest
 * segment before it and the start of the nearest one after it, ignoring
 * `selfId`. Snapping stays inside, so it never creates an overlap.
 */
export function laneBounds(
  segments: readonly { id: string; startMs: number | null; endMs: number | null }[],
  startMs: number,
  endMs: number,
  selfId: string | null = null,
): { lo?: number; hi?: number } {
  let lo: number | undefined;
  let hi: number | undefined;
  for (const seg of segments) {
    if (seg.id === selfId || seg.startMs === null || seg.endMs === null) continue;
    if (seg.endMs <= startMs && (lo === undefined || seg.endMs > lo)) lo = seg.endMs;
    if (seg.startMs >= endMs && (hi === undefined || seg.startMs < hi)) hi = seg.startMs;
  }
  return { ...(lo === undefined ? {} : { lo }), ...(hi === undefined ? {} : { hi }) };
}
