// Pure grouping and keyboard navigation for the Search panel.
import type { SearchRow } from "../api";

export type Scope = "all" | "texts" | "lexicon";

export const SCOPES: { scope: Scope; label: string }[] = [
  { scope: "all", label: "All" },
  { scope: "texts", label: "Texts" },
  { scope: "lexicon", label: "Lexicon" },
];

export type Group = { key: string; title: string; rows: SearchRow[] };

const isLexicon = (r: SearchRow) => r.kind === "lexeme" || r.kind === "sense";

/** A row opens something only when it names a text. */
export const navigable = (r: SearchRow): boolean => r.textId !== null;

/** Hits grouped by text, then Lexicon, then Other. Groups appear in order of
 * their best hit and rows keep the rank order the session returned. */
export function groupRows(rows: SearchRow[]): Group[] {
  const groups = new Map<string, Group>();
  for (const r of rows) {
    const [key, title] = r.textId !== null ? [`text:${r.textId}`, r.textTitle || "Untitled text"] : isLexicon(r) ? ["lexicon", "Lexicon"] : ["other", "Other"];
    let g = groups.get(key);
    if (g === undefined) groups.set(key, (g = { key, title, rows: [] }));
    g.rows.push(r);
  }
  return [...groups.values()];
}

/** The rows the arrow keys walk, in display order. Rows that open nothing are skipped. */
export const navTargets = (groups: Group[]): SearchRow[] => groups.flatMap((g) => g.rows).filter(navigable);

/** Next active index for an arrow key over `count` rows; -1 means none. Stops at the ends. */
export function moveActive(current: number, key: "ArrowDown" | "ArrowUp", count: number): number {
  if (count === 0) return -1;
  if (key === "ArrowDown") return Math.min(current + 1, count - 1);
  return current < 0 ? 0 : Math.max(current - 1, 0);
}

export function clockTime(ms: number): string {
  const s = Math.floor(Math.max(ms, 0) / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** "0:05 · Utterance" for a segment hit; empty for anything else. */
export function whereLabel(r: SearchRow): string {
  if (r.kind !== "segment") return "";
  return [r.startMs === null ? null : clockTime(r.startMs), r.tierName].filter((x) => x !== null && x !== "").join(" · ");
}
