// Agreement between two tracks of one text: two annotators' transcriptions of
// the same recording, a machine track against a hand one, or a converted track
// against the one corrected by hand. Segments are paired by how much they
// overlap (ELAN's inter-annotator reliability pairs the same way), and the
// pairs give how often the labels are identical, Cohen's kappa when the labels
// are categories, the sound error rate when they are transcriptions, and how
// far apart the boundaries are. Pure; no DOM.
import { sounds } from "../sounds/ipa.ts";

export type Seg = {
  id: string;
  tier: string;
  startMs: number | null;
  endMs: number | null;
  content: string;
};
type Timed = { id: string; start: number; end: number; label: string };

export type Pair = {
  a: Timed;
  b: Timed;
  /** Overlap as a share of the two segments' combined extent, 0–1. */
  overlap: number;
  same: boolean;
  /** Sounds that differ (insertions, deletions, substitutions; a word boundary counts as one). */
  distance: number;
  /** Sounds in a's label, the error rate's denominator. */
  length: number;
};

export type Row =
  | { kind: "pair"; pair: Pair }
  | { kind: "a"; seg: Timed }
  | { kind: "b"; seg: Timed };

export type Agreement = {
  rows: Row[];
  pairs: number;
  onlyA: number;
  onlyB: number;
  /** Matched pairs whose labels are identical. */
  identical: number;
  /** Cohen's kappa over the pairs and the unmatched segments (labelled ∅), null when it cannot be worked out. */
  kappa: number | null;
  /** Distinct labels across both tracks: kappa means something only when they are few. */
  labels: number;
  /** Sound error rate over the pairs: differing sounds over a's sounds. */
  errorRate: number | null;
  startDiff: { mean: number; median: number } | null;
  endDiff: { mean: number; median: number } | null;
};

/** ELAN's default: two segments are one pair when they overlap by 60% of their combined extent. */
export const MIN_OVERLAP = 0.6;
/** Kappa is offered only when the labels look like categories. */
export const MAX_CATEGORIES = 40;
export const EMPTY = "∅";

export const label = (s: string): string =>
  s.trim().replace(/\s+/gu, " ").normalize("NFC");

function timed(segments: readonly Seg[], tier: string): Timed[] {
  const out: Timed[] = [];
  for (const s of segments) {
    if (
      s.tier !== tier ||
      s.startMs === null ||
      s.endMs === null ||
      !(s.endMs > s.startMs)
    )
      continue;
    out.push({
      id: s.id,
      start: s.startMs,
      end: s.endMs,
      label: label(s.content),
    });
  }
  return out.sort((x, y) => x.start - y.start || x.end - y.end);
}

/** The label as units to compare: each word's sounds, a space between words. */
export function units(text: string): string[] {
  const out: string[] = [];
  for (const w of text.split(" ")) {
    if (w === "") continue;
    if (out.length > 0) out.push(" ");
    const s = sounds(w);
    out.push(...(s.length > 0 ? s : [...w]));
  }
  return out;
}

/** Levenshtein distance over two lists of units. */
export function editDistance(
  a: readonly string[],
  b: readonly string[],
): number {
  let prev = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    const cur = [i];
    for (let j = 1; j <= b.length; j++)
      cur.push(
        Math.min(
          prev[j]! + 1,
          cur[j - 1]! + 1,
          prev[j - 1]! + (a[i - 1] === b[j - 1] ? 0 : 1),
        ),
      );
    prev = cur;
  }
  return prev[b.length]!;
}

const overlapOf = (x: Timed, y: Timed): number => {
  const inter = Math.min(x.end, y.end) - Math.max(x.start, y.start);
  if (inter <= 0) return 0;
  return inter / (Math.max(x.end, y.end) - Math.min(x.start, y.start));
};

const middle = (sorted: readonly number[]): number => {
  const m = sorted.length >> 1;
  return sorted.length % 2 === 1
    ? sorted[m]!
    : (sorted[m - 1]! + sorted[m]!) / 2;
};

function spread(values: number[]): { mean: number; median: number } | null {
  if (values.length === 0) return null;
  values.sort((x, y) => x - y);
  return {
    mean: values.reduce((s, v) => s + v, 0) / values.length,
    median: middle(values),
  };
}

/** Cohen's kappa for two raters' labels of the same items. */
export function cohensKappa(
  items: readonly (readonly [string, string])[],
): number | null {
  const n = items.length;
  if (n === 0) return null;
  const a = new Map<string, number>();
  const b = new Map<string, number>();
  let agree = 0;
  for (const [x, y] of items) {
    if (x === y) agree += 1;
    a.set(x, (a.get(x) ?? 0) + 1);
    b.set(y, (b.get(y) ?? 0) + 1);
  }
  let pe = 0;
  for (const [k, c] of a) pe += (c / n) * ((b.get(k) ?? 0) / n);
  if (pe >= 1) return null;
  return (agree / n - pe) / (1 - pe);
}

/** Pairs the two tracks' timed segments, the pairs overlapping most first,
 * each segment in at most one pair, and works out the agreement. */
export function agreement(
  segments: readonly Seg[],
  tierA: string,
  tierB: string,
  minOverlap = MIN_OVERLAP,
): Agreement {
  const as = timed(segments, tierA);
  const bs = timed(segments, tierB);
  const candidates: { i: number; j: number; o: number }[] = [];
  // Both lists are sorted by start, so a b that starts more than the longest b
  // before x starts cannot reach x, nor any later x.
  const longest = bs.reduce((m, s) => Math.max(m, s.end - s.start), 0);
  let from = 0;
  for (let i = 0; i < as.length; i++) {
    const x = as[i]!;
    while (from < bs.length && bs[from]!.start < x.start - longest) from += 1;
    for (let j = from; j < bs.length && bs[j]!.start < x.end; j++) {
      const o = overlapOf(x, bs[j]!);
      if (o >= minOverlap) candidates.push({ i, j, o });
    }
  }
  candidates.sort((p, q) => q.o - p.o || p.i - q.i || p.j - q.j);
  const usedA = new Set<number>();
  const usedB = new Set<number>();
  const pairs: Pair[] = [];
  for (const { i, j, o } of candidates) {
    if (usedA.has(i) || usedB.has(j)) continue;
    usedA.add(i);
    usedB.add(j);
    const a = as[i]!;
    const b = bs[j]!;
    const ua = units(a.label);
    const ub = units(b.label);
    pairs.push({
      a,
      b,
      overlap: o,
      same: a.label === b.label,
      distance: editDistance(ua, ub),
      length: ua.length,
    });
  }
  const rows: Row[] = [
    ...pairs.map((pair): Row => ({ kind: "pair", pair })),
    ...as
      .filter((_, i) => !usedA.has(i))
      .map((seg): Row => ({ kind: "a", seg })),
    ...bs
      .filter((_, j) => !usedB.has(j))
      .map((seg): Row => ({ kind: "b", seg })),
  ];
  rows.sort((x, y) => startOf(x) - startOf(y));
  const items: [string, string][] = rows.map((r) =>
    r.kind === "pair"
      ? [r.pair.a.label, r.pair.b.label]
      : r.kind === "a"
        ? [r.seg.label, EMPTY]
        : [EMPTY, r.seg.label],
  );
  const labels = new Set(items.flat().filter((l) => l !== EMPTY)).size;
  const length = pairs.reduce((s, p) => s + p.length, 0);
  return {
    rows,
    pairs: pairs.length,
    onlyA: as.length - pairs.length,
    onlyB: bs.length - pairs.length,
    identical: pairs.filter((p) => p.same).length,
    kappa: labels <= MAX_CATEGORIES ? cohensKappa(items) : null,
    labels,
    errorRate:
      length === 0 ? null : pairs.reduce((s, p) => s + p.distance, 0) / length,
    startDiff: spread(pairs.map((p) => Math.abs(p.a.start - p.b.start))),
    endDiff: spread(pairs.map((p) => Math.abs(p.a.end - p.b.end))),
  };
}

export const startOf = (r: Row): number =>
  r.kind === "pair" ? Math.min(r.pair.a.start, r.pair.b.start) : r.seg.start;

/** Rows that show a difference: unmatched segments and pairs whose labels differ. */
export const differs = (r: Row): boolean => r.kind !== "pair" || !r.pair.same;

const pct = (part: number, whole: number) =>
  `${Math.round((100 * part) / whole)}%`;
const plural = (n: number, one: string, many = `${one}s`) =>
  `${n} ${n === 1 ? one : many}`;

/** Landis and Koch's words for a kappa. */
export function kappaWords(k: number): string {
  if (k < 0) return "worse than chance";
  if (k <= 0.2) return "slight";
  if (k <= 0.4) return "fair";
  if (k <= 0.6) return "moderate";
  if (k <= 0.8) return "substantial";
  return "almost perfect";
}

/** The summary in words, a line each. */
export function agreementLines(
  r: Agreement,
  nameA: string,
  nameB: string,
): string[] {
  if (r.pairs + r.onlyA + r.onlyB === 0)
    return [`Neither «${nameA}» nor «${nameB}» has timed segments.`];
  const lines: string[] = [];
  const unmatched = [
    r.onlyA > 0 ? `${r.onlyA} only on «${nameA}»` : "",
    r.onlyB > 0 ? `${r.onlyB} only on «${nameB}»` : "",
  ].filter((s) => s !== "");
  lines.push(
    `${plural(r.pairs, "pair")} of segments overlapping by ${Math.round(MIN_OVERLAP * 100)}% or more${unmatched.length > 0 ? `; ${unmatched.join(", ")}` : ""}.`,
  );
  if (r.pairs > 0) {
    lines.push(
      `Labels identical in ${r.identical} of ${r.pairs} pairs (${pct(r.identical, r.pairs)}).`,
    );
    if (r.errorRate !== null)
      lines.push(
        `Sound error rate ${(r.errorRate * 100).toFixed(1)}%: sounds inserted, left out or changed, against «${nameA}».`,
      );
  }
  if (r.kappa !== null)
    lines.push(
      `Cohen's kappa ${r.kappa.toFixed(2)} (${kappaWords(r.kappa)}), over ${plural(r.labels, "label")}, unmatched segments counted against ${EMPTY}.`,
    );
  else if (r.labels > MAX_CATEGORIES)
    lines.push(
      `No kappa: ${r.labels} different labels are transcriptions rather than categories, so the error rate says more.`,
    );
  if (r.startDiff !== null && r.endDiff !== null)
    lines.push(
      `Boundaries apart by ${Math.round(r.startDiff.mean)} ms at the start and ${Math.round(r.endDiff.mean)} ms at the end on average (medians ${Math.round(r.startDiff.median)} and ${Math.round(r.endDiff.median)} ms).`,
    );
  return lines;
}

const secs = (ms: number) => (ms / 1000).toFixed(3);
const cell = (s: string) => s.replace(/[\t\r\n]+/gu, " ");

/** Every row as a tab-separated table for a spreadsheet or R. */
export function agreementTsv(
  r: Agreement,
  nameA: string,
  nameB: string,
): string {
  const head = [
    `${nameA} start`,
    `${nameA} end`,
    nameA,
    `${nameB} start`,
    `${nameB} end`,
    nameB,
    "overlap",
    "identical",
    "sounds apart",
  ].map(cell);
  const lines = [head.join("\t")];
  for (const row of r.rows) {
    if (row.kind === "pair") {
      const { a, b } = row.pair;
      lines.push(
        [
          secs(a.start),
          secs(a.end),
          cell(a.label),
          secs(b.start),
          secs(b.end),
          cell(b.label),
          row.pair.overlap.toFixed(2),
          row.pair.same ? "yes" : "no",
          String(row.pair.distance),
        ].join("\t"),
      );
    } else if (row.kind === "a")
      lines.push(
        [
          secs(row.seg.start),
          secs(row.seg.end),
          cell(row.seg.label),
          "",
          "",
          "",
          "",
          "",
          "",
        ].join("\t"),
      );
    else
      lines.push(
        [
          "",
          "",
          "",
          secs(row.seg.start),
          secs(row.seg.end),
          cell(row.seg.label),
          "",
          "",
          "",
        ].join("\t"),
      );
  }
  return lines.join("\n") + "\n";
}

/** The tracks worth comparing: transcription tracks (not translations) with timed, labelled segments, in their order. */
export function comparable<T extends { id: string; role?: string | null }>(
  tiers: readonly T[],
  segments: readonly Seg[],
): T[] {
  const count = counts(segments);
  return tiers.filter(
    (t) => t.role !== "translation" && (count.get(t.id) ?? 0) > 0,
  );
}

function counts(segments: readonly Seg[]): Map<string, number> {
  const count = new Map<string, number>();
  for (const s of segments)
    if (
      s.startMs !== null &&
      s.endMs !== null &&
      s.endMs > s.startMs &&
      s.content.trim() !== ""
    )
      count.set(s.tier, (count.get(s.tier) ?? 0) + 1);
  return count;
}

/** The two tracks to compare first: the two comparable ones with most timed segments, in their order. */
export function defaultTracks(
  tiers: readonly { id: string; role?: string | null }[],
  segments: readonly Seg[],
): [string, string] | null {
  const count = counts(segments);
  const ids = comparable(tiers, segments).map((t) => t.id);
  if (ids.length < 2) return null;
  const top = [...ids]
    .sort((x, y) => (count.get(y) ?? 0) - (count.get(x) ?? 0))
    .slice(0, 2);
  return ids.filter((id) => top.includes(id)) as [string, string];
}
