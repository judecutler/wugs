// Where the texts were recorded: coordinates read from a text's details
// (`coordinates`, `gps`, or `latitude` and `longitude`), as an archive's
// deposit form and an OLAC record ask for them. Pure: no DOM.

export type Place = { title: string; lat: number; lon: number };

/** The detail keys that hold a "lat, lon" pair, in the order they are read. */
export const PAIR_KEYS = ["coordinates", "gps", "latlon", "lat_lon", "location", "geo"];
const LAT_KEYS = ["latitude", "lat"];
const LON_KEYS = ["longitude", "lon", "long", "lng"];
/** Every detail that holds a coordinate: summing texts by one of them says nothing. */
export const COORDINATE_KEYS: readonly string[] = [...PAIR_KEYS, ...LAT_KEYS, ...LON_KEYS];

/** One coordinate: decimal degrees (signed, or with N S E W) or degrees, minutes and seconds (8°30′15″S). */
const ONE = /([-−+]?\d+(?:\.\d+)?)\s*(?:[°º˚]\s*(?:(\d+(?:\.\d+)?)\s*['′’]\s*)?(?:(\d+(?:\.\d+)?)\s*["″”]\s*)?)?\s*([NSEWnsew])?/gu;

function readOne(m: RegExpExecArray): { value: number; hemi: string | null } {
  const deg = Number(m[1]!.replace("−", "-"));
  const min = m[2] === undefined ? 0 : Number(m[2]);
  const sec = m[3] === undefined ? 0 : Number(m[3]);
  const magnitude = Math.abs(deg) + min / 60 + sec / 3600;
  const hemi = m[4] === undefined ? null : m[4].toUpperCase();
  const negative = deg < 0 || m[1]!.startsWith("−") || hemi === "S" || hemi === "W";
  return { value: negative ? -magnitude : magnitude, hemi };
}

/** "lat, lon" in any common writing, or null when it does not read as a place on Earth. */
export function parseCoordinates(text: string): { lat: number; lon: number } | null {
  const found = [...text.matchAll(ONE)].filter((m) => m[0].trim() !== "");
  if (found.length !== 2) return null;
  let [a, b] = found.map(readOne) as [ReturnType<typeof readOne>, ReturnType<typeof readOne>];
  // Written longitude first (E 115, S 8)? Hemispheres say which is which.
  if ((a.hemi === "E" || a.hemi === "W") && (b.hemi === "N" || b.hemi === "S")) [a, b] = [b, a];
  if ((a.hemi !== null && a.hemi !== "N" && a.hemi !== "S") || (b.hemi !== null && b.hemi !== "E" && b.hemi !== "W")) return null;
  return inRange(a.value, b.value);
}

const inRange = (lat: number, lon: number) => (Number.isFinite(lat) && Number.isFinite(lon) && Math.abs(lat) <= 90 && Math.abs(lon) <= 180 ? { lat, lon } : null);

/** One coordinate alone (a `latitude` detail). */
function single(text: string): number | null {
  const m = [...text.matchAll(ONE)].filter((x) => x[0].trim() !== "");
  return m.length === 1 ? readOne(m[0]!).value : null;
}

/** Where one text was recorded, from its details, or null. */
export function placeOf(meta: readonly (readonly [string, string])[]): { lat: number; lon: number } | null {
  const get = (keys: readonly string[]) => keys.map((k) => meta.find(([key]) => key === k)?.[1]).find((v) => v !== undefined && v.trim() !== "");
  for (const k of PAIR_KEYS) {
    const v = get([k]);
    const p = v === undefined ? null : parseCoordinates(v);
    if (p !== null) return p;
  }
  const lat = get(LAT_KEYS);
  const lon = get(LON_KEYS);
  if (lat === undefined || lon === undefined) return null;
  const a = single(lat);
  const b = single(lon);
  return a === null || b === null ? null : inRange(a, b);
}

/** Every text with a place, and how many name a place in words only. */
export function textPlaces(texts: readonly { title: string; meta: readonly (readonly [string, string])[] }[]): { places: Place[]; wordsOnly: number } {
  const places: Place[] = [];
  let wordsOnly = 0;
  for (const t of texts) {
    const p = placeOf(t.meta);
    if (p !== null) places.push({ title: t.title, ...p });
    else if (t.meta.some(([k, v]) => k === "place" && v.trim() !== "")) wordsOnly += 1;
  }
  return { places, wordsOnly };
}

/** The line under the map. */
export function placesNote(found: { places: Place[]; wordsOnly: number }, texts: number): string {
  const n = (x: number, one: string, many: string) => `${x} ${x === 1 ? one : many}`;
  const sites = new Set(found.places.map((p) => `${p.lat.toFixed(3)},${p.lon.toFixed(3)}`)).size;
  const parts = [`${n(found.places.length, "text", "texts")} of ${texts} placed, at ${n(sites, "site", "sites")}.`];
  if (found.wordsOnly > 0) parts.push(`${n(found.wordsOnly, "text names", "texts name")} a place in words only; add its coordinates as a detail (coordinates: −8.65, 115.22) to put it on the map.`);
  return parts.join(" ");
}
