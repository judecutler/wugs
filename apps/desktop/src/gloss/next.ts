// The Gloss panel's Next to gloss: the next sentence on the same track that
// still has a word to gloss, in the order sentences are numbered. Pure.

type Seg = { id: string; tier: string; startMs: number | null };

/** The next of `still` after `current` on its track (timed ones by start, untimed after, in the text's order), wrapping round; null when none is left but the current one. */
export function nextStill<T extends Seg>(segments: readonly T[], current: T, still: ReadonlySet<string>): T | null {
  const order = segments
    .map((s, i) => ({ s, i }))
    .filter(({ s }) => s.tier === current.tier)
    .sort((a, b) => (a.s.startMs ?? Number.MAX_SAFE_INTEGER) - (b.s.startMs ?? Number.MAX_SAFE_INTEGER) || a.i - b.i)
    .map(({ s }) => s);
  const at = order.findIndex((s) => s.id === current.id);
  for (let k = 1; k <= order.length; k++) {
    const s = order[(at + k + order.length) % order.length]!;
    if (s.id !== current.id && still.has(s.id)) return s;
  }
  return null;
}

/** What the toast says when nothing is left to go to. */
export const nothingLeft = (currentStill: boolean): string =>
  currentStill ? "This is the last sentence on its track with a word still to gloss." : "Every sentence on this track is glossed through.";
