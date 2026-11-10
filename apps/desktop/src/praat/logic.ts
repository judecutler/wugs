// Praat handoff display rules (roadmap item 46), pure so node --test runs
// them. The commands live in src-tauri/src/praat.rs; the engine that turns
// Praat's saved TextGrid into changes is wugs_legacy::praat.

export type Range = { startMs: number; endMs: number };

/** Context shown around a selected segment when it opens in Praat. */
export const PAD_MS = 1000;

/**
 * What Praat opens zoomed to: the selected segment with a second either
 * side, else what the timeline shows, kept inside the recording.
 */
export function praatRange(selected: { startMs: number | null; endMs: number | null } | null, visible: Range, durationMs: number): Range {
  const end = durationMs > 0 ? durationMs : Number.POSITIVE_INFINITY;
  const fit = (r: Range): Range => {
    const startMs = Math.max(0, Math.min(r.startMs, end));
    const endMs = Math.max(startMs + 1, Math.min(r.endMs, end));
    return { startMs, endMs };
  };
  if (selected !== null && selected.startMs !== null && selected.endMs !== null) {
    return fit({ startMs: selected.startMs - PAD_MS, endMs: selected.endMs + PAD_MS });
  }
  return fit(visible);
}

export type ReturnCounts = { retimed: number; relabelled: number; added: number; removed: number };

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** "2 segments retimed, 1 label changed", or null when Praat changed nothing wugs takes back. */
export function changeSummary(c: ReturnCounts): string | null {
  const parts: string[] = [];
  if (c.retimed > 0) parts.push(`${plural(c.retimed, "segment", "segments")} retimed`);
  if (c.relabelled > 0) parts.push(`${plural(c.relabelled, "label", "labels")} changed`);
  if (c.added > 0) parts.push(`${plural(c.added, "segment", "segments")} added`);
  if (c.removed > 0) parts.push(`${plural(c.removed, "segment", "segments")} removed`);
  return parts.length === 0 ? null : parts.join(", ");
}

/** One line per reason something from Praat stays out of wugs. */
export function notBroughtBack(reasons: [string, number][]): string[] {
  return reasons.map(([why, n]) => {
    const line = n === 1 ? why : `${why} (${n})`;
    return line.charAt(0).toUpperCase() + line.slice(1);
  });
}
