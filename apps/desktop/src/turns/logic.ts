// Turn-taking: how quickly speakers take the floor from each other in a
// conversation transcribed with a track per speaker (ELAN's usual layout).
// Each change of speaker is a transition, and its floor transfer offset is
// the new turn's start minus the previous turn's end: positive for a gap,
// negative for an overlap (Heldner and Edlund 2010; Stivers et al. 2009 found
// a mode near 200 ms across ten languages). Pure; no DOM.

export type Seg = { id: string; tier: string; startMs: number | null; endMs: number | null; content: string };
type Timed = { id: string; tier: string; start: number; end: number };

export type Transition = {
  from: string;
  to: string;
  /** The new turn's start minus the previous turn's end, ms. */
  offset: number;
  previous: Timed;
  next: Timed;
};

export type Turns = {
  transitions: Transition[];
  /** Segments that start and end inside another speaker's turn: backchannels and the like. */
  inside: number;
  /** A speaker going on after their own segment, with another segment of theirs. */
  continued: number;
  segments: number;
};

export type Summary = { n: number; mean: number; median: number; gaps: number; overlaps: number; near: number };

/** Within this of zero a transition counts as no gap and no overlap to speak of. */
export const NEAR_MS = 200;
/** The histogram's range and bin width. */
export const RANGE_MS = 2000;
export const BIN_MS = 100;

/** The transitions between the given tracks, in time order. */
export function turns(segments: readonly Seg[], tiers: readonly string[]): Turns {
  const wanted = new Set(tiers);
  const timed: Timed[] = [];
  for (const s of segments) {
    if (!wanted.has(s.tier) || s.startMs === null || s.endMs === null || !(s.endMs > s.startMs)) continue;
    timed.push({ id: s.id, tier: s.tier, start: s.startMs, end: s.endMs });
  }
  timed.sort((a, b) => a.start - b.start || a.end - b.end);
  const out: Turns = { transitions: [], inside: 0, continued: 0, segments: timed.length };
  let current: Timed | null = null;
  for (const seg of timed) {
    if (current === null) {
      current = seg;
      continue;
    }
    if (seg.tier === current.tier) {
      out.continued += 1;
      if (seg.end > current.end) current = seg;
      continue;
    }
    if (seg.end <= current.end) {
      // Wholly inside the other speaker's turn: not a change of floor.
      out.inside += 1;
      continue;
    }
    out.transitions.push({ from: current.tier, to: seg.tier, offset: seg.start - current.end, previous: current, next: seg });
    current = seg;
  }
  return out;
}

const middle = (sorted: readonly number[]): number => {
  const m = sorted.length >> 1;
  return sorted.length % 2 === 1 ? sorted[m]! : (sorted[m - 1]! + sorted[m]!) / 2;
};

export function summarize(ts: readonly Transition[]): Summary | null {
  if (ts.length === 0) return null;
  const offs = ts.map((t) => t.offset).sort((a, b) => a - b);
  return {
    n: offs.length,
    mean: offs.reduce((s, v) => s + v, 0) / offs.length,
    median: middle(offs),
    gaps: offs.filter((o) => o > NEAR_MS).length,
    overlaps: offs.filter((o) => o < -NEAR_MS).length,
    near: offs.filter((o) => Math.abs(o) <= NEAR_MS).length,
  };
}

/** Counts per 100 ms bin from −2 s to 2 s; offsets beyond either end go in the end bins. */
export function histogram(ts: readonly Transition[]): { from: number; count: number }[] {
  const bins = Array.from({ length: (2 * RANGE_MS) / BIN_MS }, (_, i) => ({ from: -RANGE_MS + i * BIN_MS, count: 0 }));
  for (const t of ts) {
    const i = Math.min(bins.length - 1, Math.max(0, Math.floor((t.offset + RANGE_MS) / BIN_MS)));
    bins[i]!.count += 1;
  }
  return bins;
}

/** Each direction (speaker A to speaker B) with its count and median offset, most frequent first. */
export function byPair(ts: readonly Transition[]): { from: string; to: string; summary: Summary }[] {
  const by = new Map<string, Transition[]>();
  for (const t of ts) {
    const k = `${t.from}\u0001${t.to}`;
    by.set(k, [...(by.get(k) ?? []), t]);
  }
  return [...by.values()].map((list) => ({ from: list[0]!.from, to: list[0]!.to, summary: summarize(list)! })).sort((a, b) => b.summary.n - a.summary.n || a.from.localeCompare(b.from) || a.to.localeCompare(b.to));
}

const ms = (v: number) => `${v > 0 ? "+" : v < 0 ? "−" : ""}${Math.abs(Math.round(v))} ms`;
const pct = (part: number, whole: number) => `${Math.round((100 * part) / whole)}%`;

/** The summary in words. */
export function turnsLines(t: Turns, s: Summary | null): string[] {
  if (s === null) return [t.segments === 0 ? "No timed segments on these tracks." : "No change of speaker between these tracks yet: each track should hold one speaker's turns."];
  const lines = [
    `${s.n} ${s.n === 1 ? "change" : "changes"} of speaker. Median offset ${ms(s.median)}, mean ${ms(s.mean)}.`,
    `${pct(s.gaps, s.n)} start after a gap of more than ${NEAR_MS} ms, ${pct(s.overlaps, s.n)} overlap the previous turn by more than ${NEAR_MS} ms, ${pct(s.near, s.n)} come within ${NEAR_MS} ms of its end.`,
  ];
  const aside = [t.inside > 0 ? `${t.inside} ${t.inside === 1 ? "segment falls" : "segments fall"} wholly inside another speaker's turn (backchannels, not counted)` : "", t.continued > 0 ? `${t.continued} ${t.continued === 1 ? "segment continues" : "segments continue"} the same speaker's turn` : ""].filter((x) => x !== "");
  if (aside.length > 0) lines.push(`${aside.join("; ")}.`);
  return lines;
}

const secs = (v: number) => (v / 1000).toFixed(3);

/** Every transition as a tab-separated table, with track names. */
export function turnsTsv(ts: readonly Transition[], name: (tier: string) => string): string {
  const rows = ["from\tto\tprevious end\tnext start\toffset ms"];
  for (const t of ts) rows.push([name(t.from), name(t.to), secs(t.previous.end), secs(t.next.start), String(Math.round(t.offset))].join("\t"));
  return rows.join("\n") + "\n";
}
