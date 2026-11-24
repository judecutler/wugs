// The Concordance's plot on a map: each recording site (a text's coordinates
// detail, as the Progress panel reads it) with its matches per 1000 words,
// for seeing where a word or form is said, as a dialect atlas would. Pure.

import { placeOf } from "../progress/places.ts";
import type { TextPlot } from "./plot.ts";

export type Site = { lat: number; lon: number; titles: string[]; words: number; hits: number };

/** The texts summed by site (coordinates to about 100 m), and how many texts have no coordinates. */
export function bySite(texts: readonly TextPlot[]): { sites: Site[]; unplaced: number } {
  const sites = new Map<string, Site>();
  let unplaced = 0;
  for (const t of texts) {
    const p = placeOf(t.meta ?? []);
    if (p === null) {
      unplaced += 1;
      continue;
    }
    const key = `${p.lat.toFixed(3)},${p.lon.toFixed(3)}`;
    const s = sites.get(key) ?? { lat: p.lat, lon: p.lon, titles: [], words: 0, hits: 0 };
    s.titles.push(t.title || "(untitled)");
    s.words += t.words;
    s.hits += t.hits;
    sites.set(key, s);
  }
  return { sites: [...sites.values()], unplaced };
}

/** Whether any text has coordinates, so the map is worth offering. */
export const hasSites = (texts: readonly TextPlot[]) => texts.some((t) => placeOf(t.meta ?? []) !== null);

/** A dot's radius as a share of the largest: by area, so twice the rate looks twice as big; none for no matches. */
export function dotShare(s: Site, sites: readonly Site[]): number {
  const rate = (x: Site) => (x.words === 0 ? 0 : x.hits / x.words);
  const top = Math.max(0, ...sites.map(rate));
  return top === 0 ? 0 : Math.sqrt(rate(s) / top);
}

/** A site's tooltip. */
export function siteTitle(s: Site): string {
  const per = s.words === 0 ? "0" : ((1000 * s.hits) / s.words).toFixed(s.hits === 0 ? 0 : 1);
  return `${s.titles.join(", ")}: ${s.hits} ${s.hits === 1 ? "match" : "matches"}, ${per} per 1000 words`;
}

/** The line above the map. */
export function sitesLine(found: { sites: Site[]; unplaced: number }): string {
  const n = (x: number, one: string, many: string) => `${x} ${x === 1 ? one : many}`;
  const said = found.sites.filter((s) => s.hits > 0).length;
  const parts = [`Found at ${n(said, "site", "sites")} of ${found.sites.length}: each dot's size is its matches per 1000 words, an empty ring none.`];
  if (found.unplaced > 0) parts.push(`${n(found.unplaced, "text has", "texts have")} no coordinates detail and ${found.unplaced === 1 ? "is" : "are"} not on the map.`);
  return parts.join(" ");
}

/** The sites as a table to paste, with a header. */
export function sitesTsv(sites: readonly Site[]): string {
  const rows = sites.map((s) => [s.lat.toFixed(5), s.lon.toFixed(5), s.titles.join("; "), String(s.words), String(s.hits), s.words === 0 ? "0" : ((1000 * s.hits) / s.words).toFixed(2)].join("\t"));
  return ["latitude\tlongitude\ttexts\twords\tmatches\tper 1000", ...rows].join("\n") + "\n";
}
