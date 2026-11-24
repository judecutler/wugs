// Speech rhythm from a track of one sound a segment (phones from a forced
// aligner, or hand-segmented vowels and consonants). Neighbouring vowels
// join into a vocalic interval and neighbouring consonants into a
// consonantal one, within a stretch of speech (a pause, a silence label or a
// segment that is not one sound ends a stretch). From the intervals:
// Ramus, Nespor and Mehler's (1999) %V, ΔV and ΔC; Dellwo's (2006)
// VarcoV and VarcoC; Grabe and Low's (2002) nPVI over vocalic intervals and
// rPVI over consonantal ones. Pure; no DOM.
import { sounds } from "../sounds/ipa.ts";
import { classOf } from "../sounds/shapes.ts";
import type { Seg } from "./rate.ts";

/** Segments further apart than this start a new stretch. */
export const JOIN_MS = 20;
/** Fewer intervals of either kind than this, and the measures say little. */
export const MIN_INTERVALS = 10;

type Kind = "V" | "C";
export type Interval = { kind: Kind; ms: number };

export type Rhythm = {
  /** Timed labelled segments on the track, and those read as one vowel or one consonant. */
  segments: number;
  classed: number;
  stretches: Interval[][];
};

/** One sound read as vowel or consonant; anything else (a word, a pause label, an unknown letter) none. */
export function kindOf(label: string): Kind | null {
  const s = sounds(label.trim().normalize("NFC"));
  if (s.length !== 1) return null;
  const c = classOf(s[0]!);
  return c === "V" || c === "C" ? c : null;
}

export function rhythm(segments: readonly Seg[], tier: string): Rhythm {
  const timed = segments
    .filter((s) => s.tier === tier && s.startMs !== null && s.endMs !== null && s.endMs > s.startMs && s.content.trim() !== "")
    .sort((a, b) => a.startMs! - b.startMs!);
  const out: Rhythm = { segments: timed.length, classed: 0, stretches: [] };
  let stretch: Interval[] = [];
  let lastEnd = -Infinity;
  const close = () => {
    if (stretch.length > 0) out.stretches.push(stretch);
    stretch = [];
  };
  for (const s of timed) {
    const kind = kindOf(s.content);
    if (kind === null || s.startMs! - lastEnd > JOIN_MS) close();
    lastEnd = Math.max(lastEnd, s.endMs!);
    if (kind === null) continue;
    out.classed += 1;
    const ms = s.endMs! - s.startMs!;
    const last = stretch.at(-1);
    if (last !== undefined && last.kind === kind) last.ms += ms;
    else stretch.push({ kind, ms });
  }
  close();
  return out;
}

const mean = (xs: readonly number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;
const sd = (xs: readonly number[]) => {
  const m = mean(xs);
  return Math.sqrt(xs.reduce((a, x) => a + (x - m) ** 2, 0) / (xs.length - 1));
};

export type Measures = { v: number; c: number; percentV: number; deltaV: number; deltaC: number; varcoV: number; varcoC: number; nPviV: number | null; rPviC: number | null };

/** The rhythm measures, or null with too few intervals. */
export function measures(r: Rhythm): Measures | null {
  const all = r.stretches.flat();
  const vs = all.filter((i) => i.kind === "V").map((i) => i.ms);
  const cs = all.filter((i) => i.kind === "C").map((i) => i.ms);
  if (vs.length < MIN_INTERVALS || cs.length < MIN_INTERVALS) return null;
  const sumV = vs.reduce((a, b) => a + b, 0);
  const sumC = cs.reduce((a, b) => a + b, 0);
  // Pairwise variability compares each interval with the next of its kind in the same stretch.
  const pairs = (kind: Kind) => r.stretches.flatMap((st) => {
    const ds = st.filter((i) => i.kind === kind).map((i) => i.ms);
    return ds.slice(1).map((d, k) => [ds[k]!, d] as const);
  });
  const pv = pairs("V");
  const pc = pairs("C");
  return {
    v: vs.length,
    c: cs.length,
    percentV: (100 * sumV) / (sumV + sumC),
    deltaV: sd(vs),
    deltaC: sd(cs),
    varcoV: (100 * sd(vs)) / mean(vs),
    varcoC: (100 * sd(cs)) / mean(cs),
    nPviV: pv.length === 0 ? null : (100 * pv.reduce((a, [x, y]) => a + Math.abs(x - y) / ((x + y) / 2), 0)) / pv.length,
    rPviC: pc.length === 0 ? null : pc.reduce((a, [x, y]) => a + Math.abs(x - y), 0) / pc.length,
  };
}

/** The section's lines; empty when the track is not one sound a segment. */
export function rhythmLines(r: Rhythm): string[] {
  if (r.segments === 0 || r.classed < r.segments / 2) return [];
  const m = measures(r);
  if (m === null) return [`Too few vowel and consonant intervals yet (${MIN_INTERVALS} of each are needed) to measure the rhythm.`];
  const f = (x: number) => x.toFixed(1);
  const lines = [
    `${m.v} vocalic and ${m.c} consonantal intervals, neighbouring vowels or consonants joined, in ${r.stretches.length} ${r.stretches.length === 1 ? "stretch" : "stretches"} of speech.`,
    `%V ${f(m.percentV)}, ΔV ${f(m.deltaV)} ms, ΔC ${f(m.deltaC)} ms (Ramus et al. 1999); VarcoV ${f(m.varcoV)}, VarcoC ${f(m.varcoC)} (Dellwo 2006).`,
  ];
  if (m.nPviV !== null || m.rPviC !== null) lines.push(`nPVI of the vowels ${m.nPviV === null ? "–" : f(m.nPviV)}, rPVI of the consonants ${m.rPviC === null ? "–" : `${f(m.rPviC)} ms`} (Grabe and Low 2002).`);
  if (r.classed < r.segments) lines.push(`${r.segments - r.classed} ${r.segments - r.classed === 1 ? "segment is" : "segments are"} not one vowel or consonant (pauses, words, unknown letters) and end a stretch.`);
  return lines;
}

/** The intervals as a table for a spreadsheet. */
export function rhythmTsv(r: Rhythm): string {
  return `stretch\tinterval\tkind\tms\n${r.stretches.flatMap((st, i) => st.map((iv, k) => [i + 1, k + 1, iv.kind, iv.ms].join("\t"))).join("\n")}\n`;
}
