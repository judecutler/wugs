// A concordance line's wider context: the sentences just before and after it
// on its own track, in the order the text reads (timed by start, untimed
// after, as examples are numbered). Pure: no DOM.

import { numbering } from "../gloss/document.ts";

export type AroundSegment = { id: string; tier: string; content: string; startMs: number | null };

/** How many sentences either side a line opens to. */
export const AROUND = 2;

/** The sentences around `id` on its track, or null when it is not in the text. */
export function around<S extends AroundSegment>(segments: readonly S[], id: string, n = AROUND): { before: S[]; after: S[] } | null {
  const me = segments.find((s) => s.id === id);
  if (me === undefined) return null;
  const track = segments.filter((s) => s.tier === me.tier && (s.id === id || s.content.trim() !== ""));
  const order = numbering(track);
  const sorted = [...track].sort((a, b) => (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0));
  const at = sorted.findIndex((s) => s.id === id);
  return { before: sorted.slice(Math.max(0, at - n), at), after: sorted.slice(at + 1, at + 1 + n) };
}
