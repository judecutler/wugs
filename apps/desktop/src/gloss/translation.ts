// The free translation under an utterance. Pure: no DOM.
export type TimedSegment = { id: string; tier: string; startMs: number | null; endMs: number | null; content: string; parent?: string | null };
export type RoleTier = { id: string; name: string; role?: string | null };

/** One translation track and the segment on it that overlaps the utterance (null when none does). */
export type TranslationLine = { tier: RoleTier; segment: TimedSegment | null };

const overlap = (a: TimedSegment, b: TimedSegment): number => {
  if (a.startMs === null || a.endMs === null || b.startMs === null || b.endMs === null) return 0;
  return Math.min(a.endMs, b.endMs) - Math.max(a.startMs, b.startMs);
};

/** For each track with role `translation`, the segment that names `selected`
 * as its parent (a FlexText free translation, a table's translation column),
 * else the one overlapping it the most. Touching edges do not overlap, so an
 * untimed utterance pairs only by parent. */
export function translationLines(selected: TimedSegment, segments: readonly TimedSegment[], tiers: readonly RoleTier[]): TranslationLine[] {
  return tiers
    .filter((t) => t.role === "translation")
    .map((tier) => {
      const own = segments.find((s) => s.tier === tier.id && s.id !== selected.id && s.parent === selected.id);
      if (own !== undefined) return { tier, segment: own };
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
}

/** There is no translation track yet: offer to add one. */
export const needsTranslationTrack = (tiers: readonly RoleTier[]): boolean => !tiers.some((t) => t.role === "translation");
