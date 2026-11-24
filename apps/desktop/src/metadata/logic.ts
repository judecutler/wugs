// Document metadata (SCROLLS.md §2): the rules the Texts panel's Details
// section follows. Keys are made the way the engine makes them
// (`wugs_core::metadata::key_of`), so what is shown is what is stored.

export type MetaField = { key: string; value: string };
export type MetaKey = { key: string; texts: number };
export type MetaView = { fields: MetaField[]; keys: MetaKey[] };
export type MetaImport = {
  by: string;
  keys: string[];
  texts: number;
  fields: number;
  unmatched: string[];
  unmatchedCount: number;
  shared: number;
};

/** The longest key the engine keeps. */
export const MAX_KEY = 40;

/** Keys most corpora describe their documents with, offered before a project has its own. */
export const SUGGESTED = ["author", "date", "genre", "speaker", "place", "source", "licence", "register", "medium"];

/**
 * A typed name as a metadata key: lower case, letters (Unicode Alphabetic, so vowel signs too) and digits of any
 * script kept, every other run one `_`, none at either end; null when it
 * is empty or starts with a digit (as the engine refuses it).
 */
export function keyOf(raw: string): string | null {
  let out = "";
  let gap = false;
  for (const c of raw.trim().toLowerCase()) {
    if (/[\p{Alphabetic}\p{N}]/u.test(c)) {
      if (gap && out !== "") out += "_";
      gap = false;
      out += c;
    } else {
      gap = true;
    }
  }
  if (out === "" || /^\p{N}/u.test(out)) return null;
  return [...out].slice(0, MAX_KEY).join("").replace(/_+$/, "");
}

/** A key as a label: `text_genre` → "Text genre". */
export function label(key: string): string {
  const words = key.replace(/_/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/**
 * Keys to offer for a new field on this text: the project's own keys, most
 * used first, then the usual ones, none the text already has.
 */
export function suggestions(view: MetaView): string[] {
  const have = new Set(view.fields.map((f) => f.key));
  const used = [...view.keys].sort((a, b) => b.texts - a.texts || a.key.localeCompare(b.key)).map((k) => k.key);
  const out: string[] = [];
  for (const k of [...used, ...SUGGESTED]) if (!have.has(k) && !out.includes(k)) out.push(k);
  return out;
}

/** How to search by this key in the Concordance's CQP query. */
export function queryHint(key: string, value: string): string {
  const escaped = value.replace(/[\\.^$|?*+()[\]{}]/g, "\\$&").replace(/"/g, '\\"');
  return `:: match.text_${key}="${escaped}"`;
}

/** What a metadata table brought in, in words. */
export function importLines(r: MetaImport): string[] {
  const lines: string[] = [];
  if (r.fields === 0 && r.unmatchedCount === 0) {
    lines.push("Every text already had these values; nothing changed.");
  } else {
    lines.push(`${r.fields} ${r.fields === 1 ? "value" : "values"} for ${r.texts} ${r.texts === 1 ? "text" : "texts"}, matched by «${r.by}».`);
  }
  if (r.keys.length > 0) lines.push(`Columns: ${r.keys.join(", ")}.`);
  if (r.unmatchedCount > 0) {
    const shown = r.unmatched.slice(0, 5).map((u) => `«${u}»`).join(", ");
    const more = r.unmatchedCount - Math.min(5, r.unmatched.length);
    lines.push(`${r.unmatchedCount} ${r.unmatchedCount === 1 ? "row names" : "rows name"} no text here: ${shown}${more > 0 ? ` and ${more} more` : ""}.`);
  }
  if (r.shared > 0) lines.push(`${r.shared} ${r.shared === 1 ? "row names a title" : "rows name titles"} several texts share; each of them got the values.`);
  return lines;
}

/** Texts picked by a detail: those whose `key` is exactly `value`. */
/** A part of the corpus: texts whose detail `key` is `value` (an empty key
 * picks none by detail), and/or one track's segments (`tier`, named `trackName`). */
export type Where = { key: string; value: string; tier?: string; trackName?: string };
/** One detail's values across the project, most texts first. */
export type MetaValues = { key: string; values: [string, number][] };
/** What a corpus tool reads: one text, the texts a detail picks, or (neither) all. */
export type Scope = { textId: string | null; within: Where | null };

/** The most values of one detail a scope picker lists. */
export const MAX_VALUES = 40;

const WHERE = "where:";
const TRACK = "track:";

/** A scope picker's option value for one track. */
export function trackValue(id: string, name: string): string {
  return TRACK + JSON.stringify([id, name]);
}

/** A scope picker's option value for a detail; a text's is its id, the whole project's "". */
export function whereValue(w: Where): string {
  return WHERE + JSON.stringify([w.key, w.value]);
}

/** A scope picker's value back as what the commands take. */
export function parseScope(value: string): Scope {
  if (value.startsWith(TRACK)) {
    try {
      const [id, name] = JSON.parse(value.slice(TRACK.length)) as [unknown, unknown];
      if (typeof id === "string" && typeof name === "string") return { textId: null, within: { key: "", value: "", tier: id, trackName: name } };
    } catch {
      // As below: the whole project.
    }
    return { textId: null, within: null };
  }
  if (value.startsWith(WHERE)) {
    try {
      const [key, v] = JSON.parse(value.slice(WHERE.length)) as [unknown, unknown];
      if (typeof key === "string" && typeof v === "string") return { textId: null, within: { key, value: v } };
    } catch {
      // A value that does not parse reads as the whole project.
    }
    return { textId: null, within: null };
  }
  return { textId: value === "" ? null : value, within: null };
}

/** An option's words: `Genre: news (3 texts)`. */
export function whereLabel(w: Where, texts: number): string {
  return `${label(w.key)}: ${w.value} (${texts} ${texts === 1 ? "text" : "texts"})`;
}

/** The picked texts in a sentence: `texts whose genre is news`. */
export function whereWords(w: Where): string {
  if (w.tier !== undefined) return `the «${w.trackName ?? w.tier}» track`;
  return `texts whose ${label(w.key).toLowerCase()} is «${w.value}»`;
}

/** The options a scope picker offers after the texts: each detail's values, at most MAX_VALUES each, details with one value for every text left out. */
export function whereOptions(values: MetaValues[], textCount: number): { key: string; options: { value: string; label: string }[] }[] {
  return values
    .filter((m) => !(m.values.length === 1 && m.values[0]![1] === textCount && textCount > 1))
    .map((m) => ({
      key: m.key,
      options: m.values.slice(0, MAX_VALUES).map(([value, n]) => ({ value: whereValue({ key: m.key, value }), label: whereLabel({ key: m.key, value }, n) })),
    }));
}
