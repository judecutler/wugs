// The work log under the Progress panel: what was done day by day, read
// from the op log (src-tauri/src/worklog.rs). Wording and table here,
// npm-tested.

export type Day = {
  date: string;
  segments: number;
  transcribed: number;
  translated: number;
  analysed: number;
  glossed: number;
  entries: number;
  senses: number;
  notes: number;
  importedTexts: number;
  importedEntries: number;
  machine: number;
  ops: number;
  devices: number;
};

type Key = Exclude<keyof Day, "date" | "ops" | "devices">;

/** The columns the log can show, in order; a column shows only when some day has it. */
export const WORK_COLUMNS: { key: Key; label: string; title: string }[] = [
  { key: "segments", label: "Segments", title: "Segments made by hand" },
  { key: "transcribed", label: "Transcribed", title: "Sentences whose text was typed" },
  { key: "translated", label: "Translated", title: "Translations added" },
  { key: "analysed", label: "Words analysed", title: "Words broken into morphs" },
  { key: "glossed", label: "Glosses", title: "Morphs whose gloss was typed" },
  { key: "entries", label: "Entries", title: "Lexicon entries made by hand" },
  { key: "senses", label: "Senses", title: "Senses made by hand" },
  { key: "notes", label: "Notes", title: "Notes on sentences" },
  { key: "importedTexts", label: "Texts imported", title: "Texts brought in from a file" },
  { key: "importedEntries", label: "Entries imported", title: "Lexicon entries brought in from a file" },
  { key: "machine", label: "Machine", title: "Segments a machine segmented or suggested for" },
];

/** The columns some day in `days` has. */
export const workColumns = (days: readonly Day[]) => WORK_COLUMNS.filter((c) => days.some((d) => d[c.key] > 0));

/** "Mon 24 Nov 2026" for `2026-11-24`, read as a calendar day. */
export function dayLabel(date: string): string {
  const [y, m, d] = date.split("-").map(Number);
  if (!y || !m || !d) return date;
  const at = new Date(Date.UTC(y, m - 1, d));
  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  return `${days[at.getUTCDay()]} ${d} ${months[m - 1]} ${y}`;
}

/** The line over the table: the days listed and what the last week holds. */
export function worklogLine(days: readonly Day[], today: string): string {
  if (days.length === 0) return "Nothing has been written in this project yet.";
  const weekAgo = shift(today, -6);
  const week = days.filter((d) => d.date >= weekAgo && d.date <= today);
  const sum = (k: Key) => week.reduce((a, d) => a + d[k], 0);
  const parts = [
    [sum("transcribed"), "sentence transcribed", "sentences transcribed"],
    [sum("translated"), "translation", "translations"],
    [sum("analysed"), "word analysed", "words analysed"],
    [sum("glossed"), "gloss", "glosses"],
    [sum("entries"), "new entry", "new entries"],
  ]
    .filter(([n]) => (n as number) > 0)
    .map(([n, one, many]) => `${(n as number).toLocaleString("en")} ${n === 1 ? one : many}`);
  const lastWeek = week.length === 0 ? "Nothing was done in the last seven days." : parts.length === 0 ? `Work on ${week.length === 1 ? "1 day" : `${week.length} days`} in the last seven, none of it transcribing, glossing or the lexicon.` : `In the last seven days: ${parts.join(", ")}, on ${week.length === 1 ? "1 day" : `${week.length} days`}.`;
  return `${lastWeek} The table lists the ${days.length === 1 ? "day" : `${days.length} latest days`} with any work, each thing counted once a day however often it was edited.`;
}

/** A date `n` days from `date` (both `YYYY-MM-DD`). */
export function shift(date: string, n: number): string {
  const [y, m, d] = date.split("-").map(Number);
  const at = new Date(Date.UTC(y ?? 1970, (m ?? 1) - 1, (d ?? 1) + n));
  return at.toISOString().slice(0, 10);
}

/** Tab-separated, every column, for a spreadsheet or a report. */
export function worklogTsv(days: readonly Day[]): string {
  return [["date", ...WORK_COLUMNS.map((c) => c.label.toLowerCase()), "devices"].join("\t"), ...days.map((d) => [d.date, ...WORK_COLUMNS.map((c) => d[c.key]), d.devices].join("\t"))].join("\n");
}
