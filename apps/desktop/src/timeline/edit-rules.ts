// When Split and Merge can run, pure and unit-tested: the toolbar buttons
// and the S / M keys ask the same rule, so a disabled button and a key that
// does nothing always agree, and the button's title says why.

type Timed = { id: string; startMs: number | null; endMs: number | null };

export type EditRules = {
  /** Why Split cannot run now, or null when it can. */
  split: string | null;
  /** Why Merge with next cannot run now, or null when it can. */
  merge: string | null;
  /** The segment Merge joins the selected one with. */
  next: string | null;
};

/** The next timed segment after `segId` on its lane, in time order (ties by id). */
export function nextTimed(lane: readonly Timed[], segId: string): string | null {
  const ordered = lane
    .filter((s) => s.startMs !== null && s.endMs !== null)
    .slice()
    .sort((a, b) => a.startMs! - b.startMs! || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  const i = ordered.findIndex((s) => s.id === segId);
  return i < 0 ? null : ordered[i + 1]?.id ?? null;
}

/**
 * What Split and Merge can do: both need a recording and a selected timed
 * segment; Merge also needs a timed next segment on the same track.
 * `selected` is the selected segment, `lane` the segments of its track.
 */
export function editRules(hasRecording: boolean, selected: Timed | null, lane: readonly Timed[]): EditRules {
  if (!hasRecording) {
    const why = "Load a recording first.";
    return { split: why, merge: why, next: null };
  }
  if (selected === null || selected.startMs === null || selected.endMs === null) {
    const why = "Select a segment on the timeline first.";
    return { split: why, merge: why, next: null };
  }
  const next = nextTimed(lane, selected.id);
  return { split: null, merge: next === null ? "There is no next segment on this track to merge with." : null, next };
}

/** How Auto-segment cuts a recording: one segment per utterance or per word. */
export type Granularity = "utterance" | "word";

/** The remembered choice, utterance unless "word" was stored. */
export function parseGranularity(stored: string | null): Granularity {
  return stored === "word" ? "word" : "utterance";
}
