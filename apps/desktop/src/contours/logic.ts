// The Pitch contours panel's rules: each timed segment's F0 at ten points
// spread evenly through it (5%, 15% … 95%, src-tauri/src/measure.rs), in Hz
// or in semitones from the speaker's median, and each label's mean contour,
// the time-normalized picture tone and intonation studies draw.

export type ContourPoint = {
  segment: string;
  label: string;
  startMs: number;
  endMs: number;
  contour: (number | null)[];
};

export type Unit = "hz" | "st";

/** A token's contour in the unit shown; null where the voice is off. */
export type Line = { point: ContourPoint; group: string; values: (number | null)[] };

export type Mean = {
  label: string;
  /** Tokens with at least one voiced point. */
  count: number;
  /** The mean at each point over the tokens voiced there; null where none is. */
  values: (number | null)[];
};

export const POSITIONS = [5, 15, 25, 35, 45, 55, 65, 75, 85, 95];

/** What to call a token's group: its label as written, trimmed. */
export const groupOf = (p: ContourPoint): string => p.label.trim() || "(no label)";

/** The median of every voiced value in Hz: the reference semitones count from. */
export function reference(points: readonly ContourPoint[]): number | null {
  const all = points.flatMap((p) => p.contour.filter((v): v is number => v !== null && v > 0)).sort((a, b) => a - b);
  if (all.length === 0) return null;
  const mid = Math.floor(all.length / 2);
  return all.length % 2 === 1 ? all[mid]! : (all[mid - 1]! + all[mid]!) / 2;
}

/** Hz as semitones above (or below) `ref`: 12·log2(hz/ref). */
export const semitones = (hz: number, ref: number): number => 12 * Math.log2(hz / ref);

/** The tokens with any voiced point, in the unit shown. */
export function lines(points: readonly ContourPoint[], unit: Unit): Line[] {
  const ref = reference(points);
  return points
    .filter((p) => p.contour.some((v) => v !== null))
    .map((p) => ({
      point: p,
      group: groupOf(p),
      values: p.contour.map((v) => (v === null ? null : unit === "st" && ref !== null ? semitones(v, ref) : v)),
    }));
}

/** One mean contour per label, most tokens first, then by label. */
export function means(shown: readonly Line[]): Mean[] {
  const by = new Map<string, Line[]>();
  for (const l of shown) by.set(l.group, [...(by.get(l.group) ?? []), l]);
  return [...by.entries()]
    .map(([label, ls]) => ({
      label,
      count: ls.length,
      values: POSITIONS.map((_, i) => {
        const v = ls.map((l) => l.values[i]).filter((x): x is number => x !== null && x !== undefined);
        return v.length === 0 ? null : v.reduce((a, b) => a + b, 0) / v.length;
      }),
    }))
    .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label));
}

/** The vertical range to draw: the values with a little room, at least 2 semitones or 20 Hz tall. */
export function valueRange(shown: readonly Line[], unit: Unit): { lo: number; hi: number } {
  const v = shown.flatMap((l) => l.values.filter((x): x is number => x !== null));
  const least = unit === "st" ? 2 : 20;
  if (v.length === 0) return unit === "st" ? { lo: -6, hi: 6 } : { lo: 75, hi: 300 };
  let lo = Math.min(...v);
  let hi = Math.max(...v);
  if (hi - lo < least) {
    const mid = (hi + lo) / 2;
    lo = mid - least / 2;
    hi = mid + least / 2;
  }
  const pad = (hi - lo) * 0.06;
  return { lo: lo - pad, hi: hi + pad };
}

/** The dash pattern of the n-th mean, so curves stay apart in print. */
export const DASHES = ["", "6 3", "2 3", "8 3 2 3"];
export const dashOf = (n: number): string => DASHES[n % DASHES.length]!;

/** Every token's contour as a tab-separated table (Hz), for R or a spreadsheet. */
export function contourTable(points: readonly ContourPoint[]): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/g, " ");
  return [
    ["label", "start_s", "end_s", ...POSITIONS.map((p) => `f0_${String(p).padStart(2, "0")}pct_hz`)].join("\t"),
    ...points.map((p) => [clean(p.label), (p.startMs / 1000).toFixed(3), (p.endMs / 1000).toFixed(3), ...p.contour.map((v) => (v === null ? "" : v.toFixed(1)))].join("\t")),
  ].join("\n");
}

/** The line under the plot. */
export function contourNote(points: readonly ContourPoint[], shown: number, untimed: number, by: string, unit: Unit, ref: number | null): string {
  const parts = [`${shown} of ${points.length} segment${points.length === 1 ? "" : "s"} drawn, analysed by ${by}.`];
  const silent = points.length - shown;
  if (silent > 0) parts.push(`${silent} had no voiced stretch.`);
  if (untimed > 0) parts.push(`${untimed} without times left out.`);
  if (unit === "st" && ref !== null) parts.push(`0 st is the speaker's median, ${Math.round(ref)} Hz.`);
  return parts.join(" ");
}
