// The Facsimile panel's rules: rectangles drawn on a page image as
// fractions of it, which areas sit on which page, and what to mark next.
// Pure; no DOM.

export type Area = { source: string; x: number; y: number; w: number; h: number };
export type Page = { source: string; label: string; path: string | null };
export type Marked = { segment: string; tier: string; content: string; area: Area };
export type FacsimileView = { pages: Page[]; marked: Marked[] };
export type Point = { x: number; y: number };

/** The smallest side an area may have, as a share of the page: a click is not a drag. */
export const MIN_SIDE = 0.004;

const clamp = (v: number) => Math.min(1, Math.max(0, v));

/** The rectangle between two corners on a page, clamped to it, or null
 * when it is too small to be meant. */
export function rectFrom(a: Point, b: Point, source: string): Area | null {
  const x0 = clamp(Math.min(a.x, b.x));
  const y0 = clamp(Math.min(a.y, b.y));
  const x1 = clamp(Math.max(a.x, b.x));
  const y1 = clamp(Math.max(a.y, b.y));
  if (x1 - x0 < MIN_SIDE || y1 - y0 < MIN_SIDE) return null;
  const round = (v: number) => Math.round(v * 1e5) / 1e5;
  const x = round(x0);
  const y = round(y0);
  return { source, x, y, w: Math.min(round(x1 - x0), 1 - x), h: Math.min(round(y1 - y0), 1 - y) };
}

/** A point on the page from a pointer position and the image's box on screen. */
export function pointIn(clientX: number, clientY: number, box: { left: number; top: number; width: number; height: number }): Point {
  return { x: box.width > 0 ? (clientX - box.left) / box.width : 0, y: box.height > 0 ? (clientY - box.top) / box.height : 0 };
}

/** The areas on one page. */
export const areasOn = (v: FacsimileView, source: string): Marked[] => v.marked.filter((m) => m.area.source === source);

/** The page a segment is marked on, as an index, or -1. */
export function pageOf(v: FacsimileView, segment: string): number {
  const m = v.marked.find((x) => x.segment === segment);
  return m === undefined ? -1 : v.pages.findIndex((p) => p.source === m.area.source);
}

/** The next segment on the same track after `segment`, in the order given,
 * that has no area yet, so marking can go on line after line. */
export function nextToMark(segments: readonly { id: string; tier: string }[], v: FacsimileView, segment: string): string | null {
  const at = segments.findIndex((s) => s.id === segment);
  if (at < 0) return null;
  const tier = segments[at]!.tier;
  const marked = new Set(v.marked.map((m) => m.segment));
  for (const s of segments.slice(at + 1)) if (s.tier === tier && !marked.has(s.id)) return s.id;
  return null;
}

/** The line above the page: how many pages and how many lines are marked. */
export function facsimileLine(v: FacsimileView, segments: number): string {
  if (v.pages.length === 0) return "No page images yet. Add a scan or photo of each page to mark where every line of the text stands on it.";
  const pages = `${v.pages.length} ${v.pages.length === 1 ? "page" : "pages"}`;
  return `${pages} · ${v.marked.length} of ${segments} ${segments === 1 ? "segment" : "segments"} marked on them.`;
}

/** Zoom steps, as a share of the panel's width; "fit" is 1. */
export const ZOOMS = [0.5, 0.75, 1, 1.5, 2, 3, 4] as const;

export function zoomBy(now: number, step: 1 | -1): number {
  const i = ZOOMS.findIndex((z) => z >= now - 1e-9);
  const at = i < 0 ? ZOOMS.length - 1 : i;
  return ZOOMS[Math.min(ZOOMS.length - 1, Math.max(0, at + step))]!;
}
