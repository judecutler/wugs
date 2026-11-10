// Pointer decisions for segment editing, pure and unit-tested: what a press
// lands on, when a press becomes a drag, and where a dragged edge goes. The
// view only applies the answers.
import { timeToX, type Viewport } from "./viewport.ts";

/** Hit zone around a segment edge, px. */
export const EDGE_PX = 6;
/** Mouse travel before a press becomes a drag, px. */
export const DRAG_PX = 4;

export type Edge = "start" | "end";
export type Timed = { id: string; startMs: number | null; endMs: number | null };
export type Hit = { kind: "edge"; id: string; edge: Edge } | { kind: "body"; id: string };

export const crossedDragThreshold = (downX: number, x: number): boolean => Math.abs(x - downX) > DRAG_PX;

/**
 * What a press at screen `x` lands on. A segment narrower than three edge
 * zones is a body target for any press inside it, so a click selects it and
 * its edges can only be grabbed from just outside; the edge zones of a
 * narrow segment would otherwise cover all of it. Among segments, a narrow
 * body wins, then any edge, then any body.
 */
export function hitTest(v: Viewport, segments: readonly Timed[], x: number): Hit | null {
  let edgeHit: Hit | null = null;
  let bodyHit: Hit | null = null;
  for (const seg of segments) {
    if (seg.startMs === null || seg.endMs === null) continue;
    const x0 = timeToX(v, seg.startMs);
    const x1 = timeToX(v, seg.endMs);
    const inside = x >= x0 && x <= x1;
    if (inside && x1 - x0 < 3 * EDGE_PX) return { kind: "body", id: seg.id };
    if (edgeHit === null) {
      if (Math.abs(x0 - x) <= EDGE_PX) edgeHit = { kind: "edge", id: seg.id, edge: "start" };
      else if (Math.abs(x1 - x) <= EDGE_PX) edgeHit = { kind: "edge", id: seg.id, edge: "end" };
    }
    if (inside && bodyHit === null) bodyHit = { kind: "body", id: seg.id };
  }
  return edgeHit ?? bodyHit;
}

/** Edge time minus pointer time at the press; keeps a drag relative. */
export const grabOffset = (edgeMs: number, pointerMs: number): number => edgeMs - pointerMs;

/** The segment with `edge` dragged to follow the pointer by its grab offset,
 * never before 0 and never closer than `minMs` to the opposite edge. */
export function dragEdge(
  edge: Edge,
  startMs: number,
  endMs: number,
  pointerMs: number,
  offsetMs: number,
  minMs: number,
): { startMs: number; endMs: number } {
  const target = Math.max(0, pointerMs + offsetMs);
  return edge === "start"
    ? { startMs: Math.min(target, endMs - minMs), endMs }
    : { startMs, endMs: Math.max(target, startMs + minMs) };
}
