// Pure helpers for the Elicit panel (roadmap item 23). No DOM.

export type Filter = "todo" | "all" | "done";
export const FILTERS: { filter: Filter; label: string }[] = [
  { filter: "todo", label: "To do" },
  { filter: "done", label: "Done" },
  { filter: "all", label: "All" },
];

export type ConceptLike = { id: string; skipped: boolean; answers: unknown[] };

/** Answered, or marked as not applicable: nothing left to ask. */
export const isDone = (c: ConceptLike): boolean => c.answers.length > 0 || c.skipped;

export function filterConcepts<T extends ConceptLike>(all: T[], filter: Filter): T[] {
  if (filter === "all") return all;
  return all.filter((c) => isDone(c) === (filter === "done"));
}

/** The next concept still to do after `id` in list order, wrapping around; null when all are done. */
export function nextTodo<T extends ConceptLike>(all: T[], id: string | null): T | null {
  const start = id === null ? -1 : all.findIndex((c) => c.id === id);
  for (let k = 1; k <= all.length; k++) {
    const c = all[(start + k) % all.length];
    if (c !== undefined && !isDone(c)) return c;
  }
  return null;
}

/** "37 of 100 answered · 2 not applicable" */
export function progressLabel(s: { total: number; answered: number; skipped: number }): string {
  const parts = [`${s.answered} of ${s.total} answered`];
  if (s.skipped > 0) parts.push(`${s.skipped} not applicable`);
  return parts.join(" · ");
}

/** Share of the list that is done, 0..1. */
export const progressShare = (s: { total: number; answered: number; skipped: number }): number =>
  s.total === 0 ? 0 : Math.min(1, (s.answered + s.skipped) / s.total);

/** Existing entries worth offering for a typed form: exact lemma match, folded. */
export function exactMatches<T extends { lemma: string }>(rows: T[], form: string, limit = 3): T[] {
  const f = form.trim().normalize("NFC").toLocaleLowerCase();
  if (f === "") return [];
  return rows.filter((r) => r.lemma.normalize("NFC").toLocaleLowerCase() === f).slice(0, limit);
}

/** A segment as the Elicit panel reads it from the text on screen. */
export type SegmentLike = { id: string; tier: string; startMs: number | null; endMs: number | null; content: string };

/** The selected segment an answer can take its sound from: one of the text
 * on screen, with times. */
export function soundSegment(sel: { textId: string; segmentId: string } | null, text: { id: string; segments: SegmentLike[] } | null): SegmentLike | null {
  if (sel === null || text === null || sel.textId !== text.id) return null;
  const seg = text.segments.find((s) => s.id === sel.segmentId);
  return seg !== undefined && seg.startMs !== null && seg.endMs !== null && seg.endMs > seg.startMs ? seg : null;
}

/** The next timed segment on the same track, in time order: where the
 * session's next word was said. */
export function nextOnTrack(segments: SegmentLike[], current: SegmentLike): SegmentLike | null {
  const after = segments.filter((s) => s.tier === current.tier && s.id !== current.id && s.startMs !== null && current.startMs !== null && (s.startMs > current.startMs || (s.startMs === current.startMs && s.id > current.id)));
  after.sort((a, b) => (a.startMs ?? 0) - (b.startMs ?? 0) || (a.id < b.id ? -1 : 1));
  return after[0] ?? null;
}

/** The line saying where the next answer's sound comes from. */
export function soundLine(seg: SegmentLike | null, clock: (ms: number) => string): string {
  if (seg === null) return "Select a segment on the Timeline: the next answer takes its sound from it, and the segment after it is selected and played.";
  const what = seg.content.trim() === "" ? "" : ` «${seg.content.trim()}»`;
  return `The next answer takes its sound from ${clock(seg.startMs ?? 0)}–${clock(seg.endMs ?? 0)}${what}.`;
}

/** What exporting the word lists wrote (src-tauri/src/wordlist.rs). */
export type WordlistExport = { path: string; lists: number; concepts: number; answered: number; forms: number; skipped: number; byProfile: boolean; notes: string[] };

/** The toast after exporting the word lists. */
export function wordlistLines(r: WordlistExport): string[] {
  const n = (k: number, one: string, many: string) => `${k.toLocaleString()} ${k === 1 ? one : many}`;
  const lines = [`${n(r.forms, "form", "forms")} for ${r.answered} of ${n(r.concepts, "concept", "concepts")} written as a CLDF Wordlist, with edictor.tsv for EDICTOR and LingPy, in ${r.path}`];
  lines.push(r.byProfile ? "Segments were cut by the project's orthography profile." : "Segments were cut letter by letter; an orthography profile (the Orthography panel) cuts groups such as «ng» as one sound.");
  if (r.skipped > 0) lines.push(r.skipped === 1 ? "1 concept is marked not applicable and has no form." : `${r.skipped.toLocaleString()} concepts are marked not applicable and have no form.`);
  return [...lines, ...r.notes];
}

/** The toast after loading a word list of one's own. */
export function ownListLine(r: { summary: { name: string; total: number }; created: number; already: number; withConcepticon: number }): string {
  const n = (k: number, one: string, many: string) => `${k.toLocaleString()} ${k === 1 ? one : many}`;
  const added = r.already === 0 ? `«${r.summary.name}»: ${n(r.created, "concept", "concepts")} loaded` : `«${r.summary.name}»: ${n(r.created, "new concept", "new concepts")} added, ${r.already.toLocaleString()} already there`;
  const linked = r.withConcepticon === 0 ? " None names a Concepticon concept set, so the export for comparison has no Concepticon IDs; a Concepticon_ID column adds them." : ` ${n(r.withConcepticon, "concept is", "concepts are")} linked to Concepticon.`;
  return `${added}.${linked}`;
}

/** A semantic domain as the coverage view reads it. */
export type CoveredDomain = { id: string; depth: number; words?: number };

/** "Words in 41 of 1,792 domains (2%)", or what to do when none has any yet. */
export function coverageLine(all: readonly CoveredDomain[]): string {
  const with_ = all.filter((d) => (d.words ?? 0) > 0).length;
  if (all.length === 0) return "";
  if (with_ === 0) return `No words collected yet in any of the ${all.length.toLocaleString("en")} domains.`;
  const words = all.reduce((n, d) => n + (d.words ?? 0), 0);
  const share = Math.round((100 * with_) / all.length);
  return `${words.toLocaleString("en")} ${words === 1 ? "word" : "words"} in ${with_.toLocaleString("en")} of ${all.length.toLocaleString("en")} domains (${share === 0 ? "under 1" : share}%).`;
}

/** The first domain after `current`, in outline order, with no words yet (wrapping round), as a Rapid Word Collection session works through them. */
export function nextEmptyDomain<T extends CoveredDomain>(all: readonly T[], current: string | null): T | null {
  const at = current === null ? -1 : all.findIndex((d) => d.id === current);
  for (let k = 1; k <= all.length; k++) {
    const d = all[(at + k + all.length) % all.length]!;
    if ((d.words ?? 0) === 0) return d;
  }
  return null;
}
