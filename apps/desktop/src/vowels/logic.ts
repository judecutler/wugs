// The Vowel space panel's rules: which points plot, Lobanov normalization,
// one group per label with its mean and a one-standard-deviation ellipse,
// and the axes (F2 right to left, F1 top to bottom, as phoneticians draw it).

export type Point = {
  segment: string;
  label: string;
  startMs: number;
  endMs: number;
  f0: number | null;
  f1: number | null;
  f2: number | null;
  f3: number | null;
  /** F1 and F2 at 20% and 80% of the segment, when the backend gives them. */
  onset?: [number | null, number | null];
  offset?: [number | null, number | null];
};

/** A point that plots: both F1 and F2 known, in the units shown. */
export type Plotted = { point: Point; x: number; y: number; group: string };

export type Group = {
  label: string;
  count: number;
  /** Mean F2 (x) and F1 (y) in the units shown. */
  x: number;
  y: number;
  /** One standard deviation: the ellipse's radii along its axes and the
   * first axis's angle from the x axis (radians), when three or more points. */
  ellipse: { rx: number; ry: number; angle: number } | null;
};

/** What to call a point's group: its label as written, trimmed. */
export const groupOf = (p: Point): string => p.label.trim() || "(no label)";

const meanOf = (v: number[]) => v.reduce((a, b) => a + b, 0) / v.length;
const sdOf = (v: number[]) => {
  if (v.length < 2) return 0;
  const m = meanOf(v);
  return Math.sqrt(v.reduce((a, b) => a + (b - m) ** 2, 0) / (v.length - 1));
};

/** How F1 and F2 are shown. Hz as measured; Bark (Traunmüller 1990) for
 * the ear's scale; and three ways to compare speakers: Lobanov (1971),
 * z-scores against the speaker's own mean and spread; Nearey (1978, the
 * formant-intrinsic log-mean), each formant's log against the speaker's
 * mean log; Watt and Fabricius (2002, as modified by Fabricius et al.
 * 2009), each formant over the centroid of the speaker's triangle of /i/,
 * /a/ and an /u/ placed at /i/'s F1. */
export type Method = "hz" | "bark" | "lobanov" | "nearey" | "watt-fabricius";

export const METHODS: { id: Method; label: string; unit: string; about: string }[] = [
  { id: "hz", label: "Hz", unit: "Hz", about: "As measured" },
  { id: "bark", label: "Bark", unit: "Bark", about: "On the ear's scale (Traunmüller 1990), still one speaker's" },
  { id: "lobanov", label: "Lobanov", unit: "z", about: "Each formant as standard deviations from this speaker's mean (Lobanov 1971), so speakers can be compared" },
  { id: "nearey", label: "Nearey", unit: "log", about: "Each formant's log against this speaker's mean log (Nearey 1978), keeping the vowels' proportions" },
  { id: "watt-fabricius", label: "Watt–Fabricius", unit: "F/S", about: "Each formant over the centre of this speaker's vowel triangle (Watt and Fabricius 2002; Fabricius et al. 2009); needs vowels labelled i and a, else the frontmost-highest and the lowest" },
];

/** A stored choice, read back; the old on/off setting meant Lobanov. */
export function parseMethod(stored: string | null): Method {
  if (stored === "on") return "lobanov";
  return METHODS.some((m) => m.id === stored) ? (stored as Method) : "hz";
}

/** Hz to Bark (Traunmüller 1990). */
export const bark = (hz: number): number => (26.81 * hz) / (1960 + hz) - 0.53;

/** The points that plot, in the units of `method`. */
export function plotted(points: readonly Point[], method: Method): Plotted[] {
  const usable = points.filter((p): p is Point & { f1: number; f2: number } => p.f1 !== null && p.f2 !== null);
  const to = scale(points, method);
  return usable.map((p) => ({ point: p, ...to(p.f1, p.f2), group: groupOf(p) }));
}

/** F1 and F2 in the units shown, measured against the speaker's midpoints
 * (so a trajectory's ends share the points' scale). */
function scale(points: readonly Point[], method: Method): (f1: number, f2: number) => { x: number; y: number } {
  if (method === "hz") return (f1, f2) => ({ x: f2, y: f1 });
  if (method === "bark") return (f1, f2) => ({ x: bark(f2), y: bark(f1) });
  const usable = points.filter((p): p is Point & { f1: number; f2: number } => p.f1 !== null && p.f2 !== null && p.f1 > 0 && p.f2 > 0);
  const f1s = usable.map((p) => p.f1);
  const f2s = usable.map((p) => p.f2);
  if (method === "nearey") {
    const [l1, l2] = [meanOf(f1s.map(Math.log)), meanOf(f2s.map(Math.log))];
    return (f1, f2) => ({ x: f2 > 0 ? Math.log(f2) - l2 : 0, y: f1 > 0 ? Math.log(f1) - l1 : 0 });
  }
  if (method === "watt-fabricius") {
    const s = wattCentroid(usable);
    return (f1, f2) => (s === null ? { x: 0, y: 0 } : { x: f2 / s.f2, y: f1 / s.f1 });
  }
  const [m1, s1, m2, s2] = [meanOf(f1s), sdOf(f1s), meanOf(f2s), sdOf(f2s)];
  return (f1, f2) => ({ x: s2 === 0 ? 0 : (f2 - m2) / s2, y: s1 === 0 ? 0 : (f1 - m1) / s1 });
}

/** The centre of the speaker's vowel triangle (Fabricius et al. 2009): /i/
 * and /a/ by their labels (`i`, `iː`, `a`, `aː`, `ɑ`), else the group most
 * front and high (largest F2 − F1) and the group lowest (largest F1); /u'/
 * has /i/'s F1 for both formants. Null with fewer than two groups. */
export function wattCentroid(points: readonly (Point & { f1: number; f2: number })[]): { f1: number; f2: number; i: string; a: string } | null {
  const by = new Map<string, { f1: number[]; f2: number[] }>();
  for (const p of points) {
    const g = groupOf(p);
    const v = by.get(g) ?? { f1: [], f2: [] };
    v.f1.push(p.f1);
    v.f2.push(p.f2);
    by.set(g, v);
  }
  const means = [...by.entries()].map(([g, v]) => ({ g, f1: meanOf(v.f1), f2: meanOf(v.f2) }));
  if (means.length < 2) return null;
  const named = (names: string[]) => means.find((m) => names.includes(m.g.normalize("NFC")));
  const i = named(["i", "iː", "i:"]) ?? means.reduce((b, m) => (m.f2 - m.f1 > b.f2 - b.f1 ? m : b));
  const a = named(["a", "aː", "a:", "ɑ", "ɑː", "æ"]) ?? means.reduce((b, m) => (m.f1 > b.f1 ? m : b));
  if (i === a) return null;
  return { f1: (i.f1 + a.f1 + i.f1) / 3, f2: (i.f2 + a.f2 + i.f1) / 3, i: i.g, a: a.g };
}

export type Trajectory = { label: string; count: number; from: { x: number; y: number }; to: { x: number; y: number } };

/** Each label's mean trajectory, from F1/F2 at 20% of its segments to F1/F2
 * at 80%, over the segments where both ends were measured; most segments first. */
export function trajectories(points: readonly Point[], method: Method): Trajectory[] {
  const to = scale(points, method);
  const by = new Map<string, { from: { x: number; y: number }; to: { x: number; y: number } }[]>();
  for (const p of points) {
    const [a1, a2] = p.onset ?? [null, null];
    const [b1, b2] = p.offset ?? [null, null];
    if (a1 === null || a2 === null || b1 === null || b2 === null) continue;
    const g = groupOf(p);
    by.set(g, [...(by.get(g) ?? []), { from: to(a1, a2), to: to(b1, b2) }]);
  }
  return [...by.entries()]
    .map(([label, list]) => ({
      label,
      count: list.length,
      from: { x: meanOf(list.map((l) => l.from.x)), y: meanOf(list.map((l) => l.from.y)) },
      to: { x: meanOf(list.map((l) => l.to.x)), y: meanOf(list.map((l) => l.to.y)) },
    }))
    .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label));
}

/** How far a trajectory moves, in the units shown (Euclidean over F1 and F2). */
export const movement = (t: Trajectory): number => Math.hypot(t.to.x - t.from.x, t.to.y - t.from.y);

/** One group per label, most points first, then by label. */
export function groups(points: readonly Plotted[]): Group[] {
  const by = new Map<string, Plotted[]>();
  for (const p of points) by.set(p.group, [...(by.get(p.group) ?? []), p]);
  return [...by.entries()]
    .map(([label, ps]) => {
      const xs = ps.map((p) => p.x);
      const ys = ps.map((p) => p.y);
      const x = meanOf(xs);
      const y = meanOf(ys);
      let ellipse: Group["ellipse"] = null;
      if (ps.length >= 3) {
        const n = ps.length - 1;
        const sxx = xs.reduce((a, v) => a + (v - x) ** 2, 0) / n;
        const syy = ys.reduce((a, v) => a + (v - y) ** 2, 0) / n;
        const sxy = xs.reduce((a, v, i) => a + (v - x) * ((ys[i] ?? y) - y), 0) / n;
        // Eigenvalues of the 2×2 covariance matrix and the first one's direction.
        const half = (sxx + syy) / 2;
        const root = Math.sqrt(((sxx - syy) / 2) ** 2 + sxy ** 2);
        const l1 = half + root;
        const l2 = Math.max(0, half - root);
        ellipse = { rx: Math.sqrt(l1), ry: Math.sqrt(l2), angle: 0.5 * Math.atan2(2 * sxy, sxx - syy) };
      }
      return { label, count: ps.length, x, y, ellipse };
    })
    .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label));
}

/** A group's ellipse as points in data units (empty without one). */
export function ellipsePath(g: Group, steps = 48): { x: number; y: number }[] {
  const e = g.ellipse;
  if (e === null) return [];
  const [c, s] = [Math.cos(e.angle), Math.sin(e.angle)];
  return Array.from({ length: steps }, (_, i) => {
    const t = (2 * Math.PI * i) / steps;
    const [a, b] = [e.rx * Math.cos(t), e.ry * Math.sin(t)];
    return { x: g.x + a * c - b * s, y: g.y + a * s + b * c };
  });
}

export type Range = { lo: number; hi: number };

/** The usual vowel space in each method's units: F1 200–1000 Hz and F2
 * 500–3000 Hz, ±2.5 SD, and their like. */
const BASE: Record<Method, { x: Range; y: Range }> = {
  hz: { x: { lo: 500, hi: 3000 }, y: { lo: 200, hi: 1000 } },
  bark: { x: { lo: bark(500), hi: bark(3000) }, y: { lo: bark(200), hi: bark(1000) } },
  lobanov: { x: { lo: -2.5, hi: 2.5 }, y: { lo: -2.5, hi: 2.5 } },
  nearey: { x: { lo: -0.8, hi: 0.8 }, y: { lo: -0.8, hi: 0.8 } },
  "watt-fabricius": { x: { lo: 0.4, hi: 1.6 }, y: { lo: 0.5, hi: 1.8 } },
};

/** The axis ranges: the usual vowel space widened to hold every point, with a margin. */
export function ranges(points: readonly Plotted[], method: Method): { x: Range; y: Range } {
  const base = BASE[method];
  const widen = (r: Range, v: number[]): Range => {
    if (v.length === 0) return r;
    const lo = Math.min(r.lo, ...v);
    const hi = Math.max(r.hi, ...v);
    const pad = (hi - lo) * 0.04;
    return { lo: lo === r.lo ? lo : lo - pad, hi: hi === r.hi ? hi : hi + pad };
  };
  return { x: widen(base.x, points.map((p) => p.x)), y: widen(base.y, points.map((p) => p.y)) };
}

/** Screen position: F2 grows to the left, F1 grows downward. */
export function place(x: number, y: number, r: { x: Range; y: Range }, box: { left: number; top: number; width: number; height: number }): { px: number; py: number } {
  return {
    px: box.left + ((r.x.hi - x) / (r.x.hi - r.x.lo)) * box.width,
    py: box.top + ((y - r.y.lo) / (r.y.hi - r.y.lo)) * box.height,
  };
}

/** Round tick values inside a range, about `count` of them. */
export function ticks(r: Range, count = 5): number[] {
  const raw = (r.hi - r.lo) / count;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? 10 * mag;
  const out: number[] = [];
  for (let v = Math.ceil(r.lo / step) * step; v <= r.hi + 1e-9; v += step) out.push(Number(v.toFixed(6)));
  return out;
}

/** The points as a tab-separated table, for a spreadsheet or R. */
export function table(points: readonly Point[]): string {
  const cell = (v: number | null, places: number) => (v === null ? "" : v.toFixed(places));
  const clean = (s: string) => s.replace(/[\t\r\n]+/g, " ");
  return [
    ["label", "start_s", "end_s", "f0_hz", "f1_hz", "f2_hz", "f3_hz", "f1_20pct_hz", "f2_20pct_hz", "f1_80pct_hz", "f2_80pct_hz"].join("\t"),
    ...points.map((p) =>
      [clean(p.label), (p.startMs / 1000).toFixed(3), (p.endMs / 1000).toFixed(3), cell(p.f0, 1), cell(p.f1, 0), cell(p.f2, 0), cell(p.f3, 0), cell(p.onset?.[0] ?? null, 0), cell(p.onset?.[1] ?? null, 0), cell(p.offset?.[0] ?? null, 0), cell(p.offset?.[1] ?? null, 0)].join("\t"),
    ),
  ].join("\n");
}

/** The line under the plot. */
export function plotNote(points: readonly Point[], shown: number, untimed: number, by: string): string {
  const missing = points.length - shown;
  const parts = [`${shown} of ${points.length} segment${points.length === 1 ? "" : "s"} plotted, analysed by ${by}.`];
  if (missing > 0) parts.push(`${missing} had no F1 or F2 (silence or noise).`);
  if (untimed > 0) parts.push(`${untimed} without times left out.`);
  return parts.join(" ");
}
