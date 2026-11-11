// Edge snapping, pure and unit-tested: a segment edge drawn or dragged
// near where speech starts or ends is pulled outward onto that point, so a
// hand-drawn box does not cut off the start of a word ("th" of "three").
// Outward only: a start moves earlier, an end later, so snapping never cuts
// more speech than the person drew. Holding Alt turns it off (the view
// decides that); spans that are not ready yet mean no snapping, never a wait.

/** A stretch of speech, [startMs, endMs], in time order. */
export type Span = readonly [number, number];

/** How far an edge may jump to reach a speech edge, ms. */
export const SNAP_MS = 250;

/**
 * Where an edge at `ms` lands. A start goes to the latest speech start at or
 * before it within `tolMs`; an end to the earliest speech end at or after it.
 * `bound` keeps the jump off a neighbour: a start never goes before it, an
 * end never past it. No candidate, no move.
 */
export function snapEdge(edge: "start" | "end", ms: number, spans: readonly Span[], tolMs = SNAP_MS, bound?: number): number {
  let best = ms;
  let bestDist = Infinity;
  for (const [s, e] of spans) {
    const point = edge === "start" ? s : e;
    const dist = edge === "start" ? ms - point : point - ms;
    if (dist < 0 || dist > tolMs || dist >= bestDist) continue;
    if (bound !== undefined && (edge === "start" ? point < bound : point > bound)) continue;
    best = point;
    bestDist = dist;
  }
  return best;
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
