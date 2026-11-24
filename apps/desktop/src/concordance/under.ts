// The line shown under a concordance line when Translations is on: a
// sentence's first translation (as the Gloss panel pairs them), or, for a
// line found in a translation, the sentence it translates. Pure: no DOM.
import { translationLines, type RoleTier, type TimedSegment } from "../gloss/translation.ts";

const overlap = (a: TimedSegment, b: TimedSegment): number =>
  a.startMs === null || a.endMs === null || b.startMs === null || b.endMs === null ? 0 : Math.min(a.endMs, b.endMs) - Math.max(a.startMs, b.startMs);

/** Every segment of one text with the line to show under it; segments with nothing to show are left out. */
export function linesUnder(segments: readonly TimedSegment[], tiers: readonly RoleTier[]): Map<string, string> {
  const byId = new Map(segments.map((s) => [s.id, s]));
  const out = new Map<string, string>();
  const translations = new Set(tiers.filter((t) => t.role === "translation").map((t) => t.id));
  for (const s of segments) {
    if (s.parent) {
      const sentence = byId.get(s.parent);
      if (sentence !== undefined && sentence.content.trim() !== "") out.set(s.id, sentence.content);
      continue;
    }
    if (translations.has(s.tier)) {
      // A timed translation: the sentence it overlaps most.
      let best: TimedSegment | null = null;
      let most = 0;
      for (const o of segments) {
        if (translations.has(o.tier) || o.parent || o.content.trim() === "") continue;
        const span = overlap(s, o);
        if (span > most) [best, most] = [o, span];
      }
      if (best !== null) out.set(s.id, best.content);
      continue;
    }
    const line = translationLines(s, segments, tiers).find((l) => l.segment !== null && l.segment.content.trim() !== "");
    if (line?.segment) out.set(s.id, line.segment.content);
  }
  return out;
}
