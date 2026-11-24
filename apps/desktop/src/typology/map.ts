// A feature's map in the Typology panel: where the languages a language is
// compared with answer one Grambank or WALS question, each a dot at its
// Glottolog coordinates on Natural Earth's land (src-tauri/src/typology.rs
// `map`). Pure: the placing, colours and words; the panel draws the SVG.

import type { Code } from "./logic.ts";

export type MapPoint = { glottocode: string; name: string; lat: number; lon: number; value: string | null; me: boolean };
export type FeatureMap = { feature: string; question: string; codes: Code[]; comparedWith: string; points: MapPoint[]; unplaced: number; land: string; edition: string };

/** Okabe and Ito's colours, told apart with most kinds of colour blindness; `?` and the uncoded are grey. */
export const COLOURS = ["#0072B2", "#E69F00", "#009E73", "#CC79A7", "#D55E00", "#56B4E9", "#F0E442", "#000000"];
export const UNKNOWN = "#999999";

/** A value's colour: its place among the feature's codes. */
export function colourOf(value: string | null, codes: readonly Code[]): string {
  if (value === null || value === "?") return UNKNOWN;
  const i = codes.findIndex((c) => c.value === value);
  return i < 0 ? UNKNOWN : COLOURS[i % COLOURS.length]!;
}

export type View<P = MapPoint> = {
  /** The SVG viewBox in the land path's units (x = lon + 180, y = 90 − lat). */
  box: { x: number; y: number; w: number; h: number };
  /** Whether western longitudes are drawn east of 180° (a group across the Pacific). */
  pacific: boolean;
  /** Each point placed: `x`, `y` in the same units. */
  placed: (P & { x: number; y: number })[];
};

const span = (xs: readonly number[]) => (xs.length === 0 ? 0 : Math.max(...xs) - Math.min(...xs));

/** Where the map looks: the points' bounding box with a margin, at least
 * 24° across, and centred on the Pacific when that keeps a group together
 * (Austronesian from Madagascar to Rapa Nui). */
export function mapView<P extends Pick<MapPoint, "lat" | "lon">>(points: readonly P[], least: { x: number; y: number } = { x: 24, y: 14 }): View<P> {
  const plain = points.map((p) => p.lon + 180);
  const shifted = points.map((p) => (p.lon < 0 ? p.lon + 540 : p.lon + 180));
  const pacific = points.length > 1 && span(shifted) + 1e-9 < span(plain);
  const placed = points.map((p, i) => ({ ...p, x: pacific ? shifted[i]! : plain[i]!, y: 90 - p.lat }));
  if (placed.length === 0) return { box: { x: 0, y: 0, w: 360, h: 180 }, pacific: false, placed };
  const xs = placed.map((p) => p.x);
  const ys = placed.map((p) => p.y);
  let x0 = Math.min(...xs);
  let x1 = Math.max(...xs);
  let y0 = Math.min(...ys);
  let y1 = Math.max(...ys);
  // A margin of an eighth of the least width each side (3° on a world map).
  const margin = least.x / 8;
  const grow = (a: number, b: number, least: number): [number, number] => {
    const mid = (a + b) / 2;
    const half = Math.max(least, (b - a) * 1.15) / 2 + margin;
    return [mid - half, mid + half];
  };
  [x0, x1] = grow(x0, x1, least.x);
  [y0, y1] = grow(y0, y1, least.y);
  // Wide enough for the panel's shape: at least half as tall as it is wide, and the other way round.
  const w = x1 - x0;
  const h = y1 - y0;
  if (h < w / 2) [y0, y1] = [(y0 + y1) / 2 - w / 4, (y0 + y1) / 2 + w / 4];
  else if (w < h) [x0, x1] = [(x0 + x1) / 2 - h / 2, (x0 + x1) / 2 + h / 2];
  y0 = Math.max(0, y0);
  y1 = Math.min(180, y1);
  return { box: { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }, pacific, placed };
}

export type LegendRow = { value: string; label: string; colour: string; count: number; mine: boolean };

/** The legend: each code with how many of the compared languages answer it, in the codes' order, then `?`; `mine` marks the language's own answer. */
export function legend(m: FeatureMap): LegendRow[] {
  const me = m.points.find((p) => p.me);
  const others = m.points.filter((p) => !p.me);
  const rows: LegendRow[] = m.codes.map((c) => ({ value: c.value, label: c.label || c.value, colour: colourOf(c.value, m.codes), count: others.filter((p) => p.value === c.value).length, mine: me?.value === c.value }));
  const unknown = others.filter((p) => p.value === "?").length;
  if (unknown > 0 || me?.value === "?") rows.push({ value: "?", label: "not known", colour: UNKNOWN, count: unknown, mine: me?.value === "?" });
  return rows.filter((r) => r.count > 0 || r.mine);
}

/** The line under the map. */
export function mapNote(m: FeatureMap): string {
  const me = m.points.find((p) => p.me);
  const n = m.points.filter((p) => !p.me).length;
  const head = `${n} ${m.comparedWith} with an answer${m.unplaced > 0 ? ` (${m.unplaced} more that Glottolog does not place)` : ""}.`;
  const own = me === undefined ? " Glottolog gives the language no place, so it is not on the map." : me.value === null ? ` ${me.name}, ringed, has no answer here yet.` : ` ${me.name} is ringed.`;
  return `${head}${own} Land from Natural Earth; places from Glottolog; answers from ${m.edition}.`;
}
