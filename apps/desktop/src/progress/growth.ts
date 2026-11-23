// Vocabulary growth (src-tauri/src/growth.rs): the different words met
// after each running word, texts in the order they were made. Pure: the
// curve's path, the ticks and the sentence under it. No DOM.

export type GrowthPoint = { tokens: number; types: number };
export type Growth = { points: GrowthPoint[]; tokens: number; types: number; texts: { title: string; at: number }[] };

/** Types after `tokens` running words, read off the curve (linear between points). */
export function typesAt(points: GrowthPoint[], tokens: number): number {
  if (points.length === 0) return 0;
  let prev = points[0]!;
  for (const p of points) {
    if (p.tokens >= tokens) {
      if (p.tokens === prev.tokens) return p.types;
      return prev.types + ((p.types - prev.types) * (tokens - prev.tokens)) / (p.tokens - prev.tokens);
    }
    prev = p;
  }
  return prev.types;
}

/** New words per 100 running words between two points of the curve. */
export function rate(points: GrowthPoint[], from: number, to: number): number {
  return to <= from ? 0 : ((typesAt(points, to) - typesAt(points, from)) * 100) / (to - from);
}

/** Fewer running words than this, and the curve says little yet. */
export const TOO_FEW = 200;

/** The sentence under the curve: how fast new words still come. */
export function growthNote(g: Growth): string {
  const n = (x: number) => Math.round(x).toLocaleString("en");
  if (g.tokens === 0) return "No transcribed words yet.";
  const head = `${n(g.tokens)} running words, ${n(g.types)} different words (${((g.types * 100) / g.tokens).toFixed(0)} per 100).`;
  if (g.tokens < TOO_FEW) return `${head} Too few yet to see whether new words are still coming as fast.`;
  const tenth = g.tokens / 10;
  const first = rate(g.points, 0, tenth);
  const last = rate(g.points, g.tokens - tenth, g.tokens);
  const trend =
    last >= first * 0.8
      ? "new words are still coming about as fast as at the start: each new text brings much that is new"
      : last <= first * 0.25
        ? "the curve is flattening: most words of these kinds of text are in"
        : "new words are coming more slowly than at the start";
  return `${head} The first tenth brought ${first.toFixed(1)} new words per 100 running words, the last tenth ${last.toFixed(1)}: ${trend}.`;
}

/** Round axis steps: 1, 2 or 5 times a power of ten, at most `most` of them. */
export function niceStep(max: number, most = 5): number {
  if (max <= 0) return 1;
  const raw = max / most;
  const pow = 10 ** Math.floor(Math.log10(raw));
  for (const m of [1, 2, 5, 10]) if (m * pow >= raw) return Math.max(1, m * pow);
  return Math.max(1, 10 * pow);
}

export type Box = { width: number; height: number; left: number; bottom: number; top: number; right: number };

/** The curve as an SVG path in `box`, tokens across and types up. */
export function growthPath(g: Growth, box: Box): string {
  const w = box.width - box.left - box.right;
  const h = box.height - box.top - box.bottom;
  const x = (t: number) => box.left + (g.tokens === 0 ? 0 : (t * w) / g.tokens);
  const y = (t: number) => box.top + h - (g.types === 0 ? 0 : (t * h) / g.types);
  return g.points.map((p, i) => `${i === 0 ? "M" : "L"}${x(p.tokens).toFixed(1)},${y(p.types).toFixed(1)}`).join(" ");
}

/** Tick values from 0 to `max` by `niceStep`. */
export function ticks(max: number): number[] {
  const step = niceStep(max);
  const out: number[] = [];
  for (let v = 0; v <= max; v += step) out.push(v);
  return out;
}
