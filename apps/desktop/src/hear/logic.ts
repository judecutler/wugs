// Hear it everywhere (roadmap 72): which recording plays a segment of a
// text. Pure: no DOM, no backend.

export type TextLink = { id: string; title: string; recording: string | null };

export type Hearing =
  /** Play; `load` is the recording to load first, or null when it is loaded. */
  | { kind: "play"; load: string | null }
  | { kind: "none"; reason: string };

/**
 * The recording a text's segments are heard from: its linked one, else the
 * loaded one while that text is the one on screen and no other text claims
 * the recording. Anything else would play (or transcribe) the wrong audio.
 */
export function recordingOf(text: TextLink | undefined, texts: readonly TextLink[], loaded: string | null, shownText: string | null): string | null {
  if (text === undefined) return null;
  if (text.recording !== null) return text.recording;
  if (loaded === null || shownText !== text.id) return null;
  return texts.some((t) => t.id !== text.id && t.recording === loaded) ? null : loaded;
}

export function planHearing(text: TextLink | undefined, texts: readonly TextLink[], loaded: string | null, shownText: string | null): Hearing {
  if (text === undefined) return { kind: "none", reason: "That text is no longer in the project." };
  const rec = recordingOf(text, texts, loaded, shownText);
  if (rec !== null) return { kind: "play", load: rec === loaded ? null : rec };
  return { kind: "none", reason: `«${text.title}» has no recording linked yet. Pick its recording once on the Timeline (in Tongues); it is remembered from then on.` };
}
