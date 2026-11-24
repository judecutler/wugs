// Durations: how long each label's segments last on one track, the numbers a
// study of vowel length, gemination or speech rate starts from. Read from the
// segments' own times, so nothing needs measuring: label the segments (a, aː,
// t, tː…) and each label gets its tokens, mean, spread and median, and two
// labels can be compared (how many times as long, and Welch's t-test). Pure;
// no DOM.

export type Seg = { id: string; tier: string; startMs: number | null; endMs: number | null; content: string };
export type Token = { segment: string; startMs: number; endMs: number; ms: number };
export type Group = { label: string; tokens: Token[]; n: number; mean: number; sd: number | null; median: number; min: number; max: number };

const median = (sorted: readonly number[]): number => {
  const m = sorted.length >> 1;
  return sorted.length % 2 === 1 ? sorted[m]! : (sorted[m - 1]! + sorted[m]!) / 2;
};

/** Each label's timed segments on the track, shortest mean first. A label is
 * the segment's text, trimmed and in NFC; empty and untimed segments are left
 * out (`untimed` counts the ones with text but no times). */
export function durationGroups(segments: readonly Seg[], tier: string): { groups: Group[]; untimed: number } {
  const by = new Map<string, Token[]>();
  let untimed = 0;
  for (const s of segments) {
    if (s.tier !== tier) continue;
    const label = s.content.trim().replace(/\s+/gu, " ").normalize("NFC");
    if (label === "") continue;
    if (s.startMs === null || s.endMs === null || !(s.endMs > s.startMs)) {
      untimed += 1;
      continue;
    }
    const list = by.get(label) ?? [];
    list.push({ segment: s.id, startMs: s.startMs, endMs: s.endMs, ms: s.endMs - s.startMs });
    by.set(label, list);
  }
  const groups = [...by.entries()].map(([label, tokens]): Group => {
    const ms = tokens.map((t) => t.ms).sort((a, b) => a - b);
    const n = ms.length;
    const mean = ms.reduce((a, b) => a + b, 0) / n;
    const sd = n < 2 ? null : Math.sqrt(ms.reduce((a, b) => a + (b - mean) ** 2, 0) / (n - 1));
    return { label, tokens, n, mean, sd, median: median(ms), min: ms[0]!, max: ms[n - 1]! };
  });
  groups.sort((a, b) => a.mean - b.mean || a.label.localeCompare(b.label));
  return { groups, untimed };
}

/** The groups with at least `min` tokens. */
export const atLeast = (groups: readonly Group[], min: number): Group[] => groups.filter((g) => g.n >= min);

/** The two labels to compare first: the two with most tokens, shorter mean first. */
export function defaultPair(groups: readonly Group[]): [string, string] | null {
  const top = [...groups].sort((a, b) => b.n - a.n || a.mean - b.mean).slice(0, 2);
  if (top.length < 2) return null;
  top.sort((a, b) => a.mean - b.mean);
  return [top[0]!.label, top[1]!.label];
}

// ---------------------------------------------------------------- statistics

/** ln Γ(x) (Lanczos, g = 7), for the incomplete beta below. */
function lnGamma(x: number): number {
  const c = [0.99999999999980993, 676.5203681218851, -1259.1392167224028, 771.32342877765313, -176.61502916214059, 12.507343278686905, -0.13857109526572012, 9.9843695780195716e-6, 1.5056327351493116e-7];
  if (x < 0.5) return Math.log(Math.PI / Math.sin(Math.PI * x)) - lnGamma(1 - x);
  x -= 1;
  let a = c[0]!;
  const t = x + 7.5;
  for (let i = 1; i < 9; i++) a += c[i]! / (x + i);
  return 0.5 * Math.log(2 * Math.PI) + (x + 0.5) * Math.log(t) - t + Math.log(a);
}

/** The continued fraction of the regularized incomplete beta (Numerical Recipes' betacf). */
function betacf(a: number, b: number, x: number): number {
  const tiny = 1e-300;
  let c = 1;
  let d = 1 - ((a + b) * x) / (a + 1);
  if (Math.abs(d) < tiny) d = tiny;
  d = 1 / d;
  let h = d;
  for (let m = 1; m <= 300; m++) {
    const m2 = 2 * m;
    let aa = (m * (b - m) * x) / ((a + m2 - 1) * (a + m2));
    d = 1 + aa * d;
    if (Math.abs(d) < tiny) d = tiny;
    c = 1 + aa / c;
    if (Math.abs(c) < tiny) c = tiny;
    d = 1 / d;
    h *= d * c;
    aa = (-(a + m) * (a + b + m) * x) / ((a + m2) * (a + m2 + 1));
    d = 1 + aa * d;
    if (Math.abs(d) < tiny) d = tiny;
    c = 1 + aa / c;
    if (Math.abs(c) < tiny) c = tiny;
    d = 1 / d;
    const del = d * c;
    h *= del;
    if (Math.abs(del - 1) < 1e-12) break;
  }
  return h;
}

/** The regularized incomplete beta I_x(a, b). */
export function incompleteBeta(x: number, a: number, b: number): number {
  if (x <= 0) return 0;
  if (x >= 1) return 1;
  const front = Math.exp(lnGamma(a + b) - lnGamma(a) - lnGamma(b) + a * Math.log(x) + b * Math.log(1 - x));
  return x < (a + 1) / (a + b + 2) ? (front * betacf(a, b, x)) / a : 1 - (front * betacf(b, a, 1 - x)) / b;
}

/** Two-sided p of Student's t with `df` degrees of freedom. */
export const tTestP = (t: number, df: number): number => incompleteBeta(df / (df + t * t), df / 2, 0.5);

export type Comparison = { a: Group; b: Group; ratio: number; t: number; df: number; p: number };

/** Welch's t-test of two labels' durations (no equal-variance assumption), and
 * how many times as long b is as a on average. Null when either has fewer
 * than two tokens or both have no spread. */
export function compare(a: Group, b: Group): Comparison | null {
  if (a.sd === null || b.sd === null || a.n < 2 || b.n < 2) return null;
  const va = a.sd ** 2 / a.n;
  const vb = b.sd ** 2 / b.n;
  if (va + vb === 0) return null;
  const t = (b.mean - a.mean) / Math.sqrt(va + vb);
  const df = (va + vb) ** 2 / (va ** 2 / (a.n - 1) + vb ** 2 / (b.n - 1));
  return { a, b, ratio: b.mean / a.mean, t, df, p: tTestP(t, df) };
}

const fmt = (ms: number): string => `${Math.round(ms)} ms`;
const pWords = (p: number): string => (p < 0.001 ? "p < 0.001" : `p = ${p < 0.01 ? p.toFixed(3) : p.toFixed(2)}`);

/** The comparison in words, as a results section would put it. */
export function compareLine(c: Comparison): string {
  const times = c.ratio >= 1 ? `«${c.b.label}» is ${c.ratio.toFixed(2)} times as long as «${c.a.label}»` : `«${c.b.label}» is ${(1 / c.ratio).toFixed(2)} times as short as «${c.a.label}»`;
  const verdict = c.p < 0.05 ? "a difference unlikely to be chance" : "no clear difference with these tokens";
  return `${times} on average (${fmt(c.b.mean)} against ${fmt(c.a.mean)}): Welch's t(${c.df.toFixed(1)}) = ${Math.abs(c.t).toFixed(2)}, ${pWords(c.p)}, ${verdict}.`;
}

/** What two labels need before they can be compared, in words. */
export function cannotCompare(a: Group | undefined, b: Group | undefined): string {
  if (a === undefined || b === undefined) return "Choose two labels to compare.";
  if (a.label === b.label) return "Choose two different labels.";
  const few = [a, b].filter((g) => g.n < 2).map((g) => `«${g.label}»`);
  if (few.length > 0) return `${few.join(" and ")} ${few.length === 1 ? "has" : "have"} one token only: a comparison needs at least two of each.`;
  return "These durations do not vary, so there is nothing to test.";
}

/** The sentence under the plot. */
export function durationsNote(groups: readonly Group[], untimed: number, shown: number): string {
  if (groups.length === 0) return untimed > 0 ? `No timed segment has a label yet; ${untimed} labelled ${untimed === 1 ? "segment has" : "segments have"} no times.` : "No segment of this track has a label yet.";
  const tokens = groups.reduce((n, g) => n + g.n, 0);
  const head = `${tokens} ${tokens === 1 ? "segment" : "segments"} under ${groups.length} ${groups.length === 1 ? "label" : "labels"}.`;
  const hidden = groups.length - shown;
  const cut = hidden > 0 ? ` ${hidden} ${hidden === 1 ? "label is" : "labels are"} left out of the plot for having too few tokens.` : "";
  const untimedNote = untimed > 0 ? ` ${untimed} labelled ${untimed === 1 ? "segment has" : "segments have"} no times and ${untimed === 1 ? "is" : "are"} left out.` : "";
  return `${head}${cut}${untimedNote} Each dot is a segment (click to hear it); the bar is the mean and one standard deviation either side.`;
}

/** Per label, for a spreadsheet. */
export function summaryTsv(groups: readonly Group[]): string {
  const rows = groups.map((g) => [g.label, g.n, Math.round(g.mean), g.sd === null ? "" : Math.round(g.sd), Math.round(g.median), Math.round(g.min), Math.round(g.max)].join("\t"));
  return [["label", "n", "mean_ms", "sd_ms", "median_ms", "min_ms", "max_ms"].join("\t"), ...rows].join("\n") + "\n";
}

/** Every token, for R or a spreadsheet. */
export function tokensTsv(groups: readonly Group[]): string {
  const rows = groups.flatMap((g) => g.tokens.map((t) => [g.label, t.segment, (t.startMs / 1000).toFixed(3), (t.endMs / 1000).toFixed(3), Math.round(t.ms)].join("\t")));
  return [["label", "segment", "start_s", "end_s", "duration_ms"].join("\t"), ...rows].join("\n") + "\n";
}

/** Round tick values for an axis from 0 to `max` ms. */
export function msTicks(max: number): number[] {
  const steps = [10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000, 10000];
  const step = steps.find((s) => max / s <= 6) ?? 20000;
  const out: number[] = [];
  for (let v = 0; v <= max + 1e-9; v += step) out.push(v);
  return out;
}
