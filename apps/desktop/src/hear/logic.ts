// Hear it everywhere (roadmap 72): which recording plays a segment of a
// text. Pure: no DOM, no backend.

export type Hearing =
  /** Play; `load` is the recording to load first, or null when it is loaded. */
  | { kind: "play"; load: string | null }
  | { kind: "none"; reason: string };

/**
 * A text plays from the recording linked to it (`recording` on its row).
 * A text with no link plays from whatever the Timeline has loaded, but only
 * while the Timeline shows that text: a recording is otherwise not known to
 * belong to it, and the wrong audio is worse than none.
 */
export function planHearing(
  text: { id: string; title: string; recording: string | null } | undefined,
  loaded: string | null,
  timelineText: string | null,
): Hearing {
  if (text === undefined) return { kind: "none", reason: "That text is no longer in the project." };
  if (text.recording !== null) return { kind: "play", load: text.recording === loaded ? null : text.recording };
  if (loaded !== null && timelineText === text.id) return { kind: "play", load: null };
  return { kind: "none", reason: `«${text.title}» has no recording linked yet. Open it on the Timeline and pick its recording; it is remembered from then on.` };
}
