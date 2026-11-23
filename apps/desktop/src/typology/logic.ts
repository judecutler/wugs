// The Typology panel's rules: which Grambank or WALS features to list and how
// to say each in words. Pure and npm-tested; the data comes from
// src-tauri/src/typology.rs.

export type Share = { value: string; label: string; count: number };
export type Code = { value: string; label: string };
export type Answer = { id: string; question: string; value: string | null; label: string | null; others: Share[]; othersCoded: number; codes: Code[] };
/** The project's own answer to a feature: a code, `?` for asked but not known, or "" with only a note. */
export type Ours = { id: string; value: string; note: string };
export type OursMap = ReadonlyMap<string, Ours>;
const NONE: OursMap = new Map();
export type Profile = {
  glottocode: string;
  name: string;
  codedAs: string | null;
  codedName: string | null;
  comparedWith: string;
  comparedCount: number;
  features: Answer[];
  edition: string;
  page: string;
};

export type Source = "grambank" | "wals";
export const SOURCES: [Source, string][] = [
  ["grambank", "Grambank"],
  ["wals", "WALS"],
];
export const sourceName = (s: Source): string => (s === "wals" ? "WALS" : "Grambank");

export type Show = "all" | "open" | "unlike" | "differs";

/** Fewer compared answers than this say too little to call a value unusual. */
export const ENOUGH = 5;

/** Whether the language answers otherwise than most of the languages it is compared with. */
export function unlike(a: Answer): boolean {
  const top = a.others[0];
  if (a.value === null || a.value === "?" || top === undefined || a.othersCoded < ENOUGH) return false;
  return a.value !== top.value && top.count * 2 > a.othersCoded;
}

const known = (v: string | null | undefined): v is string => v !== null && v !== undefined && v !== "" && v !== "?";

/** Whether the question is still open: the dataset has no answer (or `?`) and neither has the project. */
export function open(a: Answer, ours: OursMap = NONE): boolean {
  return !known(a.value) && !known(ours.get(a.id)?.value);
}

/** Whether the project answers otherwise than the dataset does. */
export function differs(a: Answer, ours: OursMap = NONE): boolean {
  const o = ours.get(a.id)?.value;
  return known(a.value) && known(o) && o !== a.value;
}

function fits(a: Answer, show: Show, ours: OursMap): boolean {
  switch (show) {
    case "all":
      return true;
    case "open":
      return open(a, ours);
    case "unlike":
      return unlike(a);
    case "differs":
      return differs(a, ours);
  }
}

/** The features to list for a choice and a filter typed in words (ID, question or the project's note, any case). */
export function shown(p: Profile, show: Show, filter: string, ours: OursMap = NONE): Answer[] {
  const f = filter.trim().toLowerCase();
  const hit = (a: Answer) => f === "" || a.id.toLowerCase().includes(f) || a.question.toLowerCase().includes(f) || (ours.get(a.id)?.note.toLowerCase().includes(f) ?? false);
  return p.features.filter((a) => fits(a, show, ours) && hit(a));
}

/** The project's answer in words. */
export function ourLine(a: Answer, o: Ours | undefined): string {
  if (o === undefined || o.value === "") return "Not answered";
  if (o.value === "?") return "Not known";
  return a.codes.find((c) => c.value === o.value)?.label ?? o.value;
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
export function profileNote(p: Profile, source: Source): string {
  const name = sourceName(source);
  const coded =
    p.codedAs === null
      ? `${name} has not coded ${p.name}: the features below are a checklist, with how related languages answer each.`
      : p.codedAs === p.glottocode
        ? `${name}'s answers for ${p.name}.`
        : `${p.name} is a dialect: these are ${name}'s answers for ${p.codedName ?? p.codedAs}.`;
  const compared = p.comparedCount === 0 ? "" : ` Compared with ${p.comparedCount.toLocaleString()} ${p.comparedWith} in ${name}.`;
  return coded + compared;
}

/** Counts for the choices. */
export function counts(p: Profile, ours: OursMap = NONE): Record<Show, number> {
  const n = (f: (a: Answer) => boolean) => p.features.filter(f).length;
  return { all: p.features.length, open: n((a) => open(a, ours)), unlike: n(unlike), differs: n((a) => differs(a, ours)) };
}

/** How many features the project has answered (a code or `?`). */
export function answeredCount(p: Profile, ours: OursMap): number {
  return p.features.filter((a) => (ours.get(a.id)?.value ?? "") !== "").length;
}

const cell = (s: string) => s.replace(/[\t\n]/g, " ");

/** The profile as a table for a spreadsheet; with the project's answers, two more columns. */
export function profileTable(p: Profile, ours?: OursMap): string {
  const head = ["ID", "Question", p.name, p.comparedWith, ...(ours ? ["This project", "Note"] : [])].join("\t");
  const rows = p.features.map((a) => {
    const o = ours?.get(a.id);
    return [a.id, cell(a.question), ownLine(a), othersLine(a), ...(ours ? [ourLine(a, o), cell(o?.note ?? "")] : [])].join("\t");
  });
  return `${[head, ...rows].join("\n")}\n`;
}

const csv = (s: string) => (/[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s);

/**
 * The project's answers as a CLDF ValueTable (`values.csv`, the shape Grambank
 * and WALS publish theirs in), for sending to the dataset's editors or adding
 * to a CLDF dataset. Features with only a note are left out.
 */
export function cldfValues(p: Profile, ours: OursMap): string {
  const rows = p.features.flatMap((a) => {
    const o = ours.get(a.id);
    if (o === undefined || o.value === "") return [];
    const code = o.value === "?" ? "" : `${a.id}-${o.value}`;
    return [[`${p.glottocode}-${a.id}`, p.glottocode, a.id, o.value, code, o.note].map(csv).join(",")];
  });
  return `${["ID,Language_ID,Parameter_ID,Value,Code_ID,Comment", ...rows].join("\n")}\n`;
}
