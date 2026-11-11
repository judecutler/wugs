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

/** Sizes in decimal units (1 MB = 1,000,000 bytes), as catalogues and download buttons give them. */
export function megabytes(bytes: number): string {
  if (bytes < 1_000_000) return `${Math.max(1, Math.round(bytes / 1000))} KB`;
  return `${(bytes / 1_000_000).toFixed(bytes < 100_000_000 ? 1 : 0)} MB`;
}

/** "1 segment", "3 segments". */
export function count(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

export function progressLabel(done: number, total: number): string {
  if (total <= 0) return `${megabytes(done)} so far`;
  return `${megabytes(done)} of ${megabytes(total)} (${Math.floor((done / total) * 100)}%)`;
}

type Summary = { ready: number; recordedNo: number; restricted: number; unchecked: number; unusable: number; emptied: number };

/** How many clips an export holds: the ready ones, plus those the records speak against when the person includes them. */
export function exportable(s: Summary, includeFlagged: boolean): number {
  return s.ready + (includeFlagged ? s.recordedNo + s.restricted : 0);
}

/** One line on what can be exported now and what is left out, in words. */
export function correctionsLine(s: Summary): string {
  const checked = s.ready + s.recordedNo + s.restricted;
  const out = s.recordedNo + s.restricted;
  const head = checked === 0
    ? "No checked suggestions to export yet."
    : out === 0
      ? `${count(checked, "checked suggestion")} ready to export.`
      : `${count(checked, "checked suggestion")}: ${s.ready} ready, ${out} left out unless you include them.`;
  const extra = [
    s.unchecked > 0 ? `${count(s.unchecked, "suggestion")} not checked yet` : null,
    s.emptied > 0 ? `${count(s.emptied, "suggestion")} cleared by hand, so there is nothing to pair` : null,
    s.unusable > 0 ? `${count(s.unusable, "clip")} without a recording or times, which cannot be exported` : null,
  ].filter((x) => x !== null);
  return extra.length === 0 ? head : `${head} Also: ${extra.join("; ")}.`;
}

/** What the project's own records say against some clips, or null when nothing does. */
export function flaggedLine(s: Summary): string | null {
  const parts = [
    s.recordedNo > 0 ? `${s.recordedNo} ${s.recordedNo === 1 ? "has a consent record" : "have consent records"} that ${s.recordedNo === 1 ? "does" : "do"} not allow AI use` : null,
    s.restricted > 0 ? `${s.restricted} ${s.restricted === 1 ? "is" : "are"} marked restricted` : null,
  ].filter((x) => x !== null);
  return parts.length === 0 ? null : `In this project's records, ${parts.join(" and ")}. They are left out unless you include them; the datasheet notes what the records say.`;
}
