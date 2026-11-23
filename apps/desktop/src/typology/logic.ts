// The Typology panel's rules: which Grambank features to list and how to say
// each in words. Pure and npm-tested; the data comes from
// src-tauri/src/grambank.rs.

export type Share = { value: string; label: string; count: number };
export type Answer = { id: string; question: string; value: string | null; label: string | null; others: Share[]; othersCoded: number };
export type Profile = {
  glottocode: string;
  name: string;
  codedAs: string | null;
  codedName: string | null;
  comparedWith: string;
  comparedCount: number;
  features: Answer[];
  edition: string;
};

export type Show = "all" | "open" | "unlike";

/** Fewer compared answers than this say too little to call a value unusual. */
export const ENOUGH = 5;

/** Whether the language answers otherwise than most of the languages it is compared with. */
export function unlike(a: Answer): boolean {
  const top = a.others[0];
  if (a.value === null || a.value === "?" || top === undefined || a.othersCoded < ENOUGH) return false;
  return a.value !== top.value && top.count * 2 > a.othersCoded;
}

/** Whether the question is still open for this language: not coded, or coded `?`. */
export function open(a: Answer): boolean {
  return a.value === null || a.value === "?";
}

/** The features to list for a choice and a filter typed in words (ID or question, any case). */
export function shown(p: Profile, show: Show, filter: string): Answer[] {
  const f = filter.trim().toLowerCase();
  return p.features.filter((a) => (show === "all" || (show === "open" ? open(a) : unlike(a))) && (f === "" || a.id.toLowerCase().includes(f) || a.question.toLowerCase().includes(f)));
}

/** The language's own answer in words. */
export function ownLine(a: Answer): string {
  if (a.value === null) return "Not coded";
  if (a.value === "?") return "Not known";
  return a.label ?? a.value;
}

/** How the compared languages answer, e.g. `present 78%, absent 22% of 290`. */
export function othersLine(a: Answer): string {
  if (a.othersCoded === 0) return "None of them coded";
  const parts = a.others.map((s) => `${s.label || s.value} ${Math.round((100 * s.count) / a.othersCoded)}%`);
  return `${parts.join(", ")} of ${a.othersCoded.toLocaleString()}`;
}

/** The line over the list, in words. */
export function profileNote(p: Profile): string {
  const coded =
    p.codedAs === null
      ? `Grambank has not coded ${p.name}: the questions below are a checklist, with how related languages answer each.`
      : p.codedAs === p.glottocode
        ? `Grambank's answers for ${p.name}.`
        : `${p.name} is a dialect: these are Grambank's answers for ${p.codedName ?? p.codedAs}.`;
  const compared = p.comparedCount === 0 ? "" : ` Compared with ${p.comparedCount.toLocaleString()} ${p.comparedWith} in Grambank.`;
  return coded + compared;
}

/** Counts for the three choices. */
export function counts(p: Profile): Record<Show, number> {
  return { all: p.features.length, open: p.features.filter(open).length, unlike: p.features.filter(unlike).length };
}

/** The profile as a table for a spreadsheet. */
export function profileTable(p: Profile): string {
  const head = ["ID", "Question", p.name, p.comparedWith].join("\t");
  const rows = p.features.map((a) => [a.id, a.question, ownLine(a), othersLine(a)].join("\t"));
  return `${[head, ...rows].join("\n")}\n`;
}

/** Grambank's page for a language. */
export const grambankPage = (glottocode: string): string => `https://grambank.clld.org/languages/${glottocode}`;
