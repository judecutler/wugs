// Measure a track (src-tauri/src/measure.rs): which track, and the report in words.

type Tier = { id: string; name: string; utterance?: boolean };
type Segment = { id: string; tier: string; startMs: number | null; endMs: number | null };

const timed = (s: Segment) => s.startMs !== null && s.endMs !== null && s.endMs > s.startMs;

/** The track to measure: the selected segment's, else the utterance track,
 * else the first with a timed segment; null when no track has one. */
export function trackToMeasure(tiers: Tier[], segments: Segment[], selected: string | null): Tier | null {
  const hasTimed = (t: Tier) => segments.some((s) => s.tier === t.id && timed(s));
  const sel = selected === null ? undefined : segments.find((s) => s.id === selected);
  const order = [tiers.find((t) => t.id === sel?.tier), tiers.find((t) => t.utterance === true), ...tiers];
  return order.find((t): t is Tier => t !== undefined && hasTimed(t)) ?? null;
}

export type MeasureReport = { path: string; measured: number; untimed: number; unvoiced: number; analysedBy: string };

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** The toast after a measurement. */
export function measuredLines(track: string, r: MeasureReport): string[] {
  const lines = [`Measured ${plural(r.measured, "segment", "segments")} of «${track}» with ${r.analysedBy}: saved as ${r.path}.`];
  if (r.unvoiced > 0) lines.push(`${plural(r.unvoiced, "segment has", "segments have")} no voiced frame, so no pitch.`);
  if (r.untimed > 0) lines.push(`${plural(r.untimed, "segment has", "segments have")} no times and ${r.untimed === 1 ? "was" : "were"} left out.`);
  return lines;
}

export type ClipsReport = { path: string; clips: number; untimed: number; /** Concordance lines whose text has no recording here. */ missing?: number };

/** The toast after concordance lines were saved as clips. */
export function hitClipsLines(r: ClipsReport): string[] {
  const lines = [`Saved ${plural(r.clips, "sentence", "sentences")} as sound files, each named after its transcription, with clips.csv listing the text and what was searched for: ${r.path}.`];
  if (r.untimed > 0) lines.push(`${plural(r.untimed, "line has", "lines have")} no times and ${r.untimed === 1 ? "was" : "were"} left out.`);
  if ((r.missing ?? 0) > 0) lines.push(`${plural(r.missing ?? 0, "line's text has", "lines' texts have")} no recording on this computer and ${r.missing === 1 ? "was" : "were"} left out: pick each text's recording in the Timeline.`);
  return lines;
}

/** The toast after saving a track's segments as sound files. */
export function clipsLines(track: string, r: ClipsReport): string[] {
  const lines = [`Saved ${plural(r.clips, "clip", "clips")} of «${track}», each named after its transcription, with clips.csv listing them: ${r.path}.`];
  if (r.untimed > 0) lines.push(`${plural(r.untimed, "segment has", "segments have")} no times and ${r.untimed === 1 ? "was" : "were"} left out.`);
  return lines;
}
