// Brown's fourteen morphemes counted in CHAT's %mor (src-tauri/src/brown.rs),
// session by session for one speaker, as KIDEVAL lists them. Pure.

export type BrownSession = { text: string; title: string; utterances: number; counts: number[] };
export type BrownTrack = { tier: string; name: string; sessions: BrownSession[] };

/** Brown's order of acquisition, as the backend counts them. */
export const MORPHEMES = [
  "Present progressive -ing",
  "in",
  "on",
  "Plural -s",
  "Irregular past",
  "Possessive 's",
  "Copula, full form",
  "Articles",
  "Regular past -ed",
  "Third person -s",
  "Irregular third person",
  "Auxiliary, full form",
  "Copula, contracted",
  "Auxiliary, contracted",
];

/** The table: a row per morpheme with its count in each session and in all. */
export function brownRows(t: BrownTrack): { morpheme: string; counts: number[]; total: number }[] {
  return MORPHEMES.map((morpheme, i) => {
    const counts = t.sessions.map((s) => s.counts[i] ?? 0);
    return { morpheme, counts, total: counts.reduce((a, b) => a + b, 0) };
  });
}

/** The line under the table. */
export function brownNote(t: BrownTrack): string {
  const rows = brownRows(t);
  const used = rows.filter((r) => r.total > 0).length;
  const firstSeen = rows.filter((r) => r.total > 0).map((r) => ({ r, at: r.counts.findIndex((c) => c > 0) }));
  const latest = firstSeen.filter((x) => x.at > 0 && x.at === Math.max(...firstSeen.map((y) => y.at)));
  const late = latest.length > 0 && t.sessions.length > 1 ? ` Last to appear: ${latest.map((x) => x.r.morpheme).join(", ")} (from «${t.sessions[latest[0]!.at]!.title}»).` : "";
  return `${t.name || "This speaker"} uses ${used} of Brown's 14 morphemes in ${t.sessions.reduce((n, s) => n + s.utterances, 0).toLocaleString("en")} utterances with %mor.${late} These are uses, as KIDEVAL counts them from %mor; Brown's mastery (90% use in obligatory contexts) needs the contexts marked, which %mor does not do. Counts are for English MOR codes.`;
}

export function brownTsv(t: BrownTrack): string {
  return [["morpheme", ...t.sessions.map((s) => s.title), "total"].join("\t"), ...brownRows(t).map((r) => [r.morpheme, ...r.counts.map(String), String(r.total)].join("\t"))].join("\n") + "\n";
}
