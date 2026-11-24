// Speech the transcription has not reached yet: stretches the recording's
// word-level speech spans cover and no segment of the track does, so a
// transcriber can find what was skipped. Pure, so npm-tested.

export type Span = readonly [number, number];

/** Words closer than `joinMs` read as one stretch of speech. */
export function stretches(words: readonly Span[], joinMs = 300): Span[] {
  const out: [number, number][] = [];
  for (const [s, e] of [...words].sort((a, b) => a[0] - b[0])) {
    const last = out[out.length - 1];
    if (last !== undefined && s - last[1] < joinMs) last[1] = Math.max(last[1], e);
    else out.push([s, e]);
  }
  return out;
}

/** The parts of each stretch of speech no segment covers, `minMs` long or more, in time order. */
export function uncovered(words: readonly Span[], segments: readonly { startMs: number | null; endMs: number | null }[], minMs = 400, joinMs = 300): Span[] {
  const covered: [number, number][] = segments
    .filter((s): s is { startMs: number; endMs: number } => s.startMs !== null && s.endMs !== null && s.endMs > s.startMs)
    .map((s): [number, number] => [s.startMs, s.endMs])
    .sort((a, b) => a[0] - b[0]);
  const out: Span[] = [];
  for (const [s, e] of stretches(words, joinMs)) {
    let at = s;
    for (const [cs, ce] of covered) {
      if (ce <= at) continue;
      if (cs >= e) break;
      if (cs > at && cs - at >= minMs) out.push([at, cs]);
      at = Math.max(at, ce);
      if (at >= e) break;
    }
    if (e - at >= minMs) out.push([at, e]);
  }
  return out;
}

/** The first gap that starts after `afterMs`, wrapping round to the first. */
export function nextGap(gaps: readonly Span[], afterMs: number): Span | null {
  return gaps.find(([s]) => s > afterMs + 1) ?? gaps[0] ?? null;
}

/** The button's words: how much speech still has no segment. */
export function gapsLabel(gaps: readonly Span[]): string {
  if (gaps.length === 0) return "No gaps";
  const secs = Math.round(gaps.reduce((n, [s, e]) => n + (e - s), 0) / 1000);
  return `Next gap (${gaps.length}, ${secs} s)`;
}
