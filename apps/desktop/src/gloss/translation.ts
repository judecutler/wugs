// The free translation under an utterance. Pure: no DOM.
export type TimedSegment = { id: string; tier: string; startMs: number | null; endMs: number | null; content: string; parent?: string | null };
export type RoleTier = { id: string; name: string; role?: string | null; kind?: string | null; parent?: string | null };

/** One translation track and its segment for the utterance (null when it has none yet). */
export type TranslationLine = { tier: RoleTier; segment: TimedSegment | null };

/** The id given for a paired track the app makes on the first translation typed. */
export const NEW_TRACK = "";

/** A track whose translations name their sentence as `parent` (ELAN's
 * symbolic association, FlexText's free translation, a table's translation
 * column) rather than lining up with it in time. */
export const isPaired = (tier: RoleTier): boolean => tier.kind === "symbolic-association";

const timed = (s: TimedSegment): boolean => s.startMs !== null && s.endMs !== null;

const overlap = (a: TimedSegment, b: TimedSegment): number => {
  if (!timed(a) || !timed(b)) return 0;
  return Math.min(a.endMs!, b.endMs!) - Math.max(a.startMs!, b.startMs!);
};

/** The translation lines for `selected`: each paired track of its own tier
 * with the segment naming it as parent, and, when it has times, each
 * time-aligned translation track with the segment overlapping it most
 * (touching edges do not overlap). An untimed utterance with no paired
 * track gets one line for a track made on the first translation typed. */
export function translationLines(selected: TimedSegment, segments: readonly TimedSegment[], tiers: readonly RoleTier[]): TranslationLine[] {
  // A segment that belongs to another (a translation, a dependent annotation) has none of its own.
  if (selected.parent) return [];
  const lines = tiers
    .filter((t) => t.role === "translation" && (isPaired(t) ? t.parent === selected.tier : timed(selected)))
    .map((tier): TranslationLine => {
      if (isPaired(tier)) return { tier, segment: segments.find((s) => s.tier === tier.id && s.parent === selected.id) ?? null };
      let best: TimedSegment | null = null;
      let bestOverlap = 0;
      for (const s of segments) {
        if (s.tier !== tier.id || s.id === selected.id) continue;
        const o = overlap(selected, s);
        if (o > bestOverlap || (o === bestOverlap && o > 0 && best !== null && (s.startMs ?? 0) < (best.startMs ?? 0))) {
          best = s;
          bestOverlap = o;
        }
      }
      return { tier, segment: best };
    });
  if (lines.length === 0 && !timed(selected)) {
    return [{ tier: { id: NEW_TRACK, name: "translation", role: "translation", kind: "symbolic-association", parent: selected.tier }, segment: null }];
  }
  return lines;
}
