// Speaking rate on one track: syllables per second, counting a syllable for
// each vowel the Sounds panel finds (a long vowel is one; a diphthong written
// as two vowels counts two). The articulation rate leaves the pauses between
// segments out; the speech rate keeps them, from the first segment's start to
// the last one's end. Pure; no DOM.
import { sounds } from "../sounds/ipa.ts";
import { classOf } from "../sounds/shapes.ts";

export type Seg = { id: string; tier: string; startMs: number | null; endMs: number | null; content: string };
export type Rate = {
  segments: number;
  syllables: number;
  /** Time inside the counted segments, and from the first start to the last end. */
  speechMs: number;
  spanMs: number;
  /** Syllables per second of each counted segment, slowest first. */
  perSegment: number[];
  /** Segments with text but no vowel found (an orthography wugs cannot read, or a pause label). */
  noVowels: number;
};

/** Vowels in a stretch of text, as syllable nuclei. */
export function syllableCount(text: string): number {
  let n = 0;
  for (const w of text.split(/\s+/u)) if (w !== "") for (const s of sounds(w)) if (classOf(s) === "V") n += 1;
  return n;
}

export function speakingRate(segments: readonly Seg[], tier: string): Rate {
  const out: Rate = { segments: 0, syllables: 0, speechMs: 0, spanMs: 0, perSegment: [], noVowels: 0 };
  let first = Infinity;
  let last = -Infinity;
  for (const s of segments) {
    if (s.tier !== tier || s.startMs === null || s.endMs === null || !(s.endMs > s.startMs) || s.content.trim() === "") continue;
    const n = syllableCount(s.content.normalize("NFC"));
    if (n === 0) {
      out.noVowels += 1;
      continue;
    }
    const ms = s.endMs - s.startMs;
    out.segments += 1;
    out.syllables += n;
    out.speechMs += ms;
    out.perSegment.push((1000 * n) / ms);
    first = Math.min(first, s.startMs);
    last = Math.max(last, s.endMs);
  }
  out.spanMs = out.segments === 0 ? 0 : last - first;
  out.perSegment.sort((a, b) => a - b);
  return out;
}

const per = (n: number) => n.toFixed(1);

/** The section's lines. */
export function rateLines(r: Rate): string[] {
  if (r.segments === 0) return [r.noVowels > 0 ? `No vowels found in the ${r.noVowels} labelled segments: the rate is read from IPA or a spelling with vowel letters.` : "No labelled timed segments on this track."];
  const articulation = (1000 * r.syllables) / r.speechMs;
  const m = r.perSegment.length >> 1;
  const median = r.perSegment.length % 2 === 1 ? r.perSegment[m]! : (r.perSegment[m - 1]! + r.perSegment[m]!) / 2;
  const lines = [
    `Articulation rate ${per(articulation)} syllables a second: ${r.syllables.toLocaleString("en")} syllables in ${(r.speechMs / 1000).toFixed(1)} s of segments (${r.segments} ${r.segments === 1 ? "segment" : "segments"}).`,
    `Per segment: median ${per(median)}, from ${per(r.perSegment[0]!)} to ${per(r.perSegment.at(-1)!)}.`,
  ];
  if (r.spanMs > r.speechMs) lines.push(`Speech rate with the pauses between segments: ${per((1000 * r.syllables) / r.spanMs)} syllables a second over ${(r.spanMs / 1000).toFixed(1)} s.`);
  if (r.noVowels > 0) lines.push(`${r.noVowels} ${r.noVowels === 1 ? "segment has" : "segments have"} no vowel wugs could read and ${r.noVowels === 1 ? "is" : "are"} left out.`);
  return lines;
}
