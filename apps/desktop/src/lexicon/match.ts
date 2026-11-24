// Pronunciations from a recording of the word list: the entries still to
// record, read aloud in the order the printed list gives them, matched one
// by one with the timed segments of a track (as Auto-segment by word makes
// them). Pure; npm-tested.

export type Word = { id: string; headword: string };
export type Spoken = { id: string; startMs: number | null; endMs: number | null; content: string };
export type Pair = { word: Word; segment: Spoken };

/** The track's timed segments in time order. */
export function spokenInOrder(segments: readonly Spoken[]): Spoken[] {
  return segments.filter((s) => s.startMs !== null && s.endMs !== null && s.endMs > s.startMs).sort((a, b) => a.startMs! - b.startMs!);
}

/** The i-th word with the i-th segment, as far as both go. */
export function pairUp(words: readonly Word[], segments: readonly Spoken[]): Pair[] {
  const spoken = spokenInOrder(segments);
  return words.slice(0, spoken.length).map((word, i) => ({ word, segment: spoken[i]! }));
}

/** The line above the pairs: how many match, and what does not line up. */
export function matchLine(words: number, segments: number): string {
  const n = (x: number, one: string, many: string) => `${x.toLocaleString("en")} ${x === 1 ? one : many}`;
  if (segments === 0) return "This track has no timed segments. Auto-segment the recording by word on the Timeline first.";
  if (words === segments) return `${n(words, "word", "words")} and ${n(segments, "segment", "segments")}: each word takes the segment in the same place. Play a few to check before adding.`;
  const more = words > segments ? `${n(words - segments, "word is", "words are")} left without a segment` : `${n(segments - words, "segment is", "segments are")} left over`;
  return `${n(words, "word", "words")} and ${n(segments, "segment", "segments")}, so ${more}: if a word was skipped or said twice, the pairs after it are off. Play them to check, untick wrong ones, or fix the segments first.`;
}
