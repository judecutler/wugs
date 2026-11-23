// The Orthography panel's rules in words. Pure and npm-tested; the profile
// is read and applied in src-tauri/src/orthography.rs.

export type Check = { columns: string[]; rules: number; problems: string[] };
export type Track = { id: string; name: string; segments: number };
export type Stored = { profile: string; check: Check; tracks: Track[] };
export type Unknown = { text: string; count: number; example: string };
export type Preview = { check: Check; segments: number; changes: { before: string; after: string }[]; unknown: Unknown[] };
export type Converted = { track: string; trackName: string; texts: number; created: number; updated: number; unchanged: number; kept: number; empty: number; unknown: Unknown[] };

/** The column to convert into: IPA when the profile has one, else its first. */
export function defaultColumn(columns: string[]): string | null {
  return columns.find((c) => c.toLowerCase() === "ipa") ?? columns[0] ?? null;
}

/**
 * The track to write: named for the column, unless a track of that name is
 * there already (the source, or one transcribed by hand), then for the
 * column and the source, e.g. "IPA from utterance".
 */
export function defaultTarget(column: string, source: string, taken: string[] = []): string {
  const has = (name: string) => [source, ...taken].some((t) => t.trim().toLowerCase() === name.trim().toLowerCase());
  if (!has(column)) return column;
  const named = `${column} from ${source}`;
  return has(named) ? `${named} (2)` : named;
}

/** The track a conversion reads first: one not named like an IPA track, the fullest. */
export function defaultSource(tracks: Track[]): string | null {
  const plain = tracks.find((t) => !/\b(ipa|phonetic|phonemic)\b/i.test(t.name));
  return (plain ?? tracks[0])?.id ?? null;
}

const n = (k: number, one: string, many: string) => `${k.toLocaleString()} ${k === 1 ? one : many}`;

/** What the profile holds, in a line. */
export function checkLine(c: Check): string {
  if (c.rules === 0 && c.columns.length === 0) return "No profile yet.";
  const into = c.columns.length === 0 ? "no column to convert into" : `converting into ${c.columns.join(", ")}`;
  return `${n(c.rules, "row", "rows")}, ${into}.`;
}

/** The letters no row covers, in a line, or "" when there are none. */
export function unknownLine(list: Unknown[], max = 12): string {
  if (list.length === 0) return "";
  const shown = list.slice(0, max).map((u) => `«${u.text}» ${u.count}× (in «${u.example}»)`);
  const more = list.length > max ? ` and ${list.length - max} more` : "";
  return `No row covers ${shown.join(", ")}${more}. They are kept as written; add rows for them and preview again.`;
}

/** The preview's summary. */
export function previewNote(p: Preview): string {
  if (p.segments === 0) return "This track has no segments to convert.";
  const first = p.changes.length < p.segments ? ` The first ${p.changes.length} are shown.` : "";
  return `${n(p.segments, "segment", "segments")} on this track.${first}`;
}

/** The toast after a conversion. */
export function convertedLines(c: Converted): string[] {
  const parts = [c.created > 0 ? `${c.created} new` : "", c.updated > 0 ? `${c.updated} rewritten` : "", c.unchanged > 0 ? `${c.unchanged} already right` : ""].filter((x) => x !== "");
  const lines = [`The «${c.trackName}» track in ${n(c.texts, "text", "texts")}: ${parts.length > 0 ? parts.join(", ") : "nothing to write"}.`];
  if (c.kept > 0) lines.push(c.kept === 1 ? "1 segment was edited by hand since the last conversion and kept as it is." : `${c.kept.toLocaleString()} segments were edited by hand since the last conversion and kept as they are.`);
  if (c.unknown.length > 0) lines.push(`${n(c.unknown.length, "letter", "letters")} had no row and stayed as written: ${c.unknown.slice(0, 8).map((u) => `«${u.text}»`).join(" ")}${c.unknown.length > 8 ? " …" : ""}`);
  return lines;
}

/** A profile as a grid: the comment lines before the header, the header's cells, and each row's cells. */
export type Grid = { comments: string[]; header: string[]; rows: string[][] };

/** A profile's text as a grid (tabs between cells); a missing header becomes Grapheme and IPA. */
export function toGrid(text: string): Grid {
  const comments: string[] = [];
  let header: string[] | null = null;
  const rows: string[][] = [];
  for (const raw of text.split(/\r?\n/)) {
    if (raw.trim() === "") continue;
    if (header === null && raw.trimStart().startsWith("#")) {
      comments.push(raw);
      continue;
    }
    if (raw.trimStart().startsWith("#")) continue;
    const cells = raw.split("\t");
    if (header === null) header = cells.map((c) => c.trim());
    else rows.push(cells);
  }
  const head = header ?? ["Grapheme", "IPA"];
  return { comments, header: head, rows: rows.map((r) => head.map((_, i) => r[i] ?? "")) };
}

/** The grid back as a profile's text; rows with every cell empty are left out. */
export function fromGrid(g: Grid): string {
  const clean = (c: string) => c.replace(/[\t\r\n]/g, " ");
  const rows = g.rows.filter((r) => r.some((c) => c.trim() !== "")).map((r) => r.map(clean).join("\t"));
  return [...g.comments, g.header.map(clean).join("\t"), ...rows].join("\n") + "\n";
}
