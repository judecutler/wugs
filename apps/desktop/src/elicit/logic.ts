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
