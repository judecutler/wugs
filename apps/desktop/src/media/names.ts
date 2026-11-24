// Recording details read from file names: archives and field kits name
// files by a convention (2026-03-12_Ana_story.wav), so a pattern such as
// {date}_{participants}_{genre} fills each recording's details at once.
// Pure, npm-tested; the Media panel writes what is read.

/** A pattern's pieces: literal text, or a detail key ({skip} reads and drops). */
export type Piece = { text: string } | { key: string };
export type Pattern = { pieces: Piece[]; keys: string[] } | { error: string };

/** A detail key as the project keeps it: lower case, other runs one `_`. */
export const detailKey = (name: string): string =>
  name
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, "_")
    .replace(/^_+|_+$/gu, "");

/** Read `{key}` placeholders and the text between them. */
export function parsePattern(pattern: string): Pattern {
  const pieces: Piece[] = [];
  const keys: string[] = [];
  const re = /\{([^{}]*)\}/gu;
  let at = 0;
  for (const m of pattern.matchAll(re)) {
    if (m.index! > at) pieces.push({ text: pattern.slice(at, m.index) });
    const key = detailKey(m[1]!);
    if (key === "") return { error: "A {…} needs a name, such as {date} or {genre}." };
    if (key !== "skip" && keys.includes(key)) return { error: `{${key}} is in the pattern twice.` };
    if (pieces.length > 0 && "key" in pieces[pieces.length - 1]!) return { error: "Two {…} need something between them, such as _ or -." };
    pieces.push({ key });
    if (key !== "skip") keys.push(key);
    at = m.index! + m[0].length;
  }
  if (at < pattern.length) pieces.push({ text: pattern.slice(at) });
  if (/[{}]/u.test(pieces.filter((p) => "text" in p).map((p) => (p as { text: string }).text).join(""))) return { error: "A { or } is not closed." };
  if (keys.length === 0) return { error: "Name at least one detail in braces, such as {date}_{participants}." };
  return { pieces, keys };
}

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");

/** A date written 20260312 or 2026.03.12 as 2026-03-12; anything else as it is. */
export function tidyDate(v: string): string {
  const m = /^(\d{4})[-._]?(\d{2})[-._]?(\d{2})$/u.exec(v);
  return m === null ? v : `${m[1]}-${m[2]}-${m[3]}`;
}

/** The details a file name gives under a pattern (its extension and any
 * folder dropped), or null when it does not fit. Pieces are read shortest
 * first, so the last one takes the rest. */
export function readName(p: Exclude<Pattern, { error: string }>, label: string): [string, string][] | null {
  const stem = (label.split(/[\\/]/u).pop() ?? label).replace(/\.[A-Za-z0-9]{1,5}$/u, "");
  const source = p.pieces.map((x, i) => ("text" in x ? escape(x.text) : i === p.pieces.length - 1 ? "(.+)" : "(.+?)")).join("");
  const m = new RegExp(`^${source}$`, "u").exec(stem);
  if (m === null) return null;
  const out: [string, string][] = [];
  let g = 1;
  for (const x of p.pieces) {
    if (!("key" in x)) continue;
    const v = m[g++]!.replace(/[_]+/gu, " ").trim();
    if (x.key !== "skip" && v !== "") out.push([x.key, x.key === "date" ? tidyDate(v.replace(/ /gu, "")) : v]);
  }
  return out;
}

/** The writes to make: each value read, for a detail that is empty (or every one, when `replace`). */
export function writesFor(read: readonly [string, string][], had: readonly [string, string][], replace: boolean): [string, string][] {
  return read.filter(([k, v]) => {
    const now = had.find(([d]) => d === k)?.[1] ?? "";
    return now !== v && (replace || now === "");
  });
}
