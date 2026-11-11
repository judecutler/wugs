// Speech assist display rules (roadmap item 27), pure so node --test runs them.
// A machine transcription stays tinted until a person edits it: the tint
// means "unverified", and its strength says how unsure the model was.

export type Machine = { model: string; confidence: number };

/** Below this mean phone confidence a suggestion counts as very unsure. */
export const VERY_UNSURE_BELOW = 0.5;

export type Tint = "unsure" | "very-unsure";

export function tint(m: Machine): Tint {
  return m.confidence < VERY_UNSURE_BELOW ? "very-unsure" : "unsure";
}

export function tintTitle(m: Machine): string {
  const pct = Math.round(Math.max(0, Math.min(1, m.confidence)) * 100);
  return `Machine transcription (${m.model}), ${pct}% sure. Edit it to mark it checked.`;
}

/** Suggestions only go to segments that are empty, timed, and short enough to hear in one go. */
export const MAX_SECONDS = 30;

type Seg = { id: string; tier: string; content: string; startMs: number | null; endMs: number | null; pos: string };

export function suggestable(s: Seg): boolean {
  return s.content.trim() === "" && s.startMs !== null && s.endMs !== null && s.endMs > s.startMs && (s.endMs - s.startMs) / 1000 <= MAX_SECONDS;
}

/** The segments of one tier a batch run would fill, in time order. */
export function batchTargets(segments: Seg[], tier: string): Seg[] {
  return segments
    .filter((s) => s.tier === tier && suggestable(s))
    .sort((a, b) => (a.startMs ?? 0) - (b.startMs ?? 0) || (a.pos < b.pos ? -1 : a.pos > b.pos ? 1 : 0));
}

export function megabytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(bytes < 100 * 1024 * 1024 ? 1 : 0)} MB`;
}

export function progressLabel(done: number, total: number): string {
  if (total <= 0) return `${megabytes(done)} so far`;
  return `${megabytes(done)} of ${megabytes(total)} (${Math.floor((done / total) * 100)}%)`;
}

type Summary = { consented: number; unrecorded: number; unchecked: number; refused: number; restricted: number; unusable: number };

/** How many checked suggestions an export holds, given the person's word on consent. */
export function exportable(s: Summary, confirmed: boolean): number {
  return s.consented + (confirmed ? s.unrecorded : 0);
}

const n = (k: number, one: string, many = `${one}s`) => `${k} ${k === 1 ? one : many}`;

/** One line on what is ready to export and what stays home, in words. */
export function correctionsLine(s: Summary): string {
  const ready = s.consented + s.unrecorded;
  const held = [
    s.unchecked > 0 ? `${n(s.unchecked, "suggestion")} not checked yet` : null,
    s.refused > 0 ? `${s.refused} refused by a consent record` : null,
    s.restricted > 0 ? `${s.restricted} restricted` : null,
    s.unusable > 0 ? `${s.unusable} without a recording` : null,
  ].filter((x) => x !== null);
  const head = ready === 0 ? "No checked suggestions to export yet." : `${n(ready, "checked suggestion")} ready to export.`;
  return held.length === 0 ? head : `${head} Staying here: ${held.join(", ")}.`;
}
