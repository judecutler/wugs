// Zipf's law in a word list: frequency against rank on log scales, and the
// exponent of the line through them (Zipf 1935: about 1 in most texts, so the
// second commonest word is half as frequent as the first). Pure; no DOM.

export type ZipfPoint = { rank: number; count: number };
export type ZipfFit = { alpha: number; intercept: number; r2: number };

/** Up to `max` ranks spread evenly on a log scale (1, 2, 3 … then wider
 * steps), so the long tail of rare words does not swamp the fit or the plot. */
export function logSpaced(n: number, max = 300): number[] {
  if (n <= 0) return [];
  const out = new Set<number>();
  const step = Math.log(n) / Math.max(1, max - 1);
  for (let i = 0; i < max; i++) out.add(Math.min(n, Math.round(Math.exp(i * step))));
  return [...out].sort((a, b) => a - b);
}

/** The points to plot and fit: counts most frequent first, read at log-spaced ranks. */
export function zipfPoints(counts: readonly number[]): ZipfPoint[] {
  const sorted = [...counts].filter((c) => c > 0).sort((a, b) => b - a);
  return logSpaced(sorted.length).map((rank) => ({ rank, count: sorted[rank - 1]! }));
}

/** Least squares through log10(count) against log10(rank): count ≈ 10^intercept · rank^−alpha. */
export function zipfFit(points: readonly ZipfPoint[]): ZipfFit | null {
  if (points.length < 3) return null;
  const xs = points.map((p) => Math.log10(p.rank));
  const ys = points.map((p) => Math.log10(p.count));
  const n = xs.length;
  const mx = xs.reduce((a, b) => a + b, 0) / n;
  const my = ys.reduce((a, b) => a + b, 0) / n;
  let sxy = 0;
  let sxx = 0;
  let syy = 0;
  for (let i = 0; i < n; i++) {
    sxy += (xs[i]! - mx) * (ys[i]! - my);
    sxx += (xs[i]! - mx) ** 2;
    syy += (ys[i]! - my) ** 2;
  }
  if (sxx === 0) return null;
  const slope = sxy / sxx;
  return { alpha: -slope, intercept: my - slope * mx, r2: syy === 0 ? 1 : (sxy * sxy) / (sxx * syy) };
}

/** The line under the plot. */
export function zipfLine(fit: ZipfFit | null, types: number, cut: boolean): string {
  if (fit === null) return "Too few different items to fit a line.";
  const reading = fit.alpha > 1.2 ? "steeper than usual: a few items take most of the uses" : fit.alpha < 0.8 ? "flatter than usual: uses spread over many items" : "close to 1, as in most texts of most languages";
  const tail = cut ? ` Only the ${types.toLocaleString("en")} commonest are counted, so the rarest are not in the plot.` : "";
  return `Zipf's exponent ${fit.alpha.toFixed(2)} (r² ${fit.r2.toFixed(2)}), ${reading}.${tail}`;
}

/** Decades for a log axis from 1 up to at least `max`. */
export function decades(max: number): number[] {
  const out = [1];
  while (out[out.length - 1]! < max) out.push(out[out.length - 1]! * 10);
  return out;
}
