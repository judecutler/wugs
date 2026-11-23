// Pure helpers for the Lexicon panel (roadmap item 22). No DOM.

/** Fold for matching: NFC, lower case, diacritics kept apart from letters. */
const fold = (s: string): string => s.normalize("NFC").toLocaleLowerCase();
/** Fold that also drops diacritics, so "kopek" finds "köpek". */
const loose = (s: string): string => fold(s).normalize("NFD").replace(/\p{M}/gu, "");

/** "yüz₂" for a numbered homograph, the bare lemma otherwise. */
export function headword(lemma: string, homograph: number | null): string {
  if (homograph === null) return lemma;
  const sub = "₀₁₂₃₄₅₆₇₈₉";
  return lemma + String(homograph).replace(/\d/g, (d) => sub[Number(d)] ?? d);
}

/** One line under the headword: the senses' glosses, numbered when there are several. */
export function glossLine(glosses: string[]): string {
  const shown = glosses.map((g) => g.trim()).filter((g) => g !== "");
  if (shown.length <= 1) return shown[0] ?? "";
  return shown.map((g, i) => `${i + 1}. ${g}`).join("  ");
}

export type DomainLike = { code: string; label: string };

/** Does a domain match what was typed? A leading digit searches the outline
 * number ("2.1" finds 2.1 and everything under it); words search the label,
 * every word must appear, diacritics and case folded. */
export function domainMatches(d: DomainLike, query: string): boolean {
  const q = query.trim();
  if (q === "") return true;
  if (/^\d/.test(q)) return d.code === q || d.code.startsWith(q.endsWith(".") ? q : q + ".");
  const label = loose(d.label);
  return loose(q).split(/\s+/).every((w) => label.includes(w));
}

/** At most `limit` matching domains, in the list's own (tree) order. */
export function filterDomains<T extends DomainLike>(all: T[], query: string, limit = 50): T[] {
  const out: T[] = [];
  for (const d of all) {
    if (domainMatches(d, query)) out.push(d);
    if (out.length >= limit) break;
  }
  return out;
}

/** A homograph number as typed: "" clears it; otherwise a whole number ≥ 1. */
export function parseHomograph(input: string): { ok: true; value: number | null } | { ok: false } {
  const t = input.trim();
  if (t === "") return { ok: true, value: null };
  if (!/^\d+$/.test(t)) return { ok: false };
  const n = Number(t);
  return n >= 1 && n <= 999 ? { ok: true, value: n } : { ok: false };
}

/** The next free homograph number among entries sharing a lemma. */
export function nextHomograph(taken: (number | null)[]): number {
  let n = 1;
  while (taken.includes(n)) n += 1;
  return n;
}

/** The notice shown when other entries share this lemma, or null. */
export function sameLemmaNote(lemma: string, own: number | null, others: (number | null)[]): string | null {
  if (others.length === 0) return null;
  const count = others.length === 1 ? "Another entry" : `${others.length} other entries`;
  const unnumbered = own === null || others.some((h) => h === null);
  return unnumbered
    ? `${count} also spelled «${lemma}». Number them as homographs (1, 2, …) to tell them apart.`
    : `${count} also spelled «${lemma}»; told apart by homograph number.`;
}

/** "used in 3 words" / "not used in any text yet". */
export function usageLabel(n: number): string {
  if (n === 0) return "not used in any text yet";
  return n === 1 ? "used in 1 word" : `used in ${n} words`;
}

/** The relation types the Lexicon offers, each as the dictionary names it. */
export const RELATIONS: { id: string; label: string }[] = [
  { id: "synonym", label: "Synonym" },
  { id: "antonym", label: "Antonym" },
  { id: "compare", label: "See also" },
  { id: "part", label: "Part of" },
  { id: "whole", label: "Has part" },
];

/** A relation's type in words, read from the entry it is shown under; a
 * type another file brought is shown as written. */
export function relationLabel(relation: string): string {
  const known = RELATIONS.find((r) => r.id === relation);
  if (known) return known.label;
  return relation.charAt(0).toUpperCase() + relation.slice(1);
}

/** The other end of a relation as a reader names it: its headword, and its
 * sense number when the relation points at one sense. */
export function relatedName(r: { otherLemma: string; otherHomograph: number | null; otherSense: number | null }): string {
  const hw = headword(r.otherLemma, r.otherHomograph);
  return r.otherSense === null ? hw : `${hw} (sense ${r.otherSense})`;
}

/** Which entries the list shows: every one, or those with work left to do. */
export type ListShow = "all" | "gloss" | "category" | "unused" | `pos:${string}`;
export type ListRow = { glosses: readonly string[]; posOfSpeech: string; used: boolean };

export const LIST_SHOWS: readonly { id: ListShow; label: string; title: string; done: string }[] = [
  { id: "all", label: "Every entry", title: "List every entry", done: "No entries match." },
  { id: "gloss", label: "Needs a gloss", title: "Entries with a sense that has no gloss yet, such as words added from the Frequencies list", done: "Every entry found has a gloss." },
  { id: "category", label: "Needs a category", title: "Entries with no category (part of speech) yet", done: "Every entry found has a category." },
  { id: "unused", label: "Not in any text", title: "Entries no glossed word in a text is linked to yet", done: "Every entry found is linked from a text." },
];

/** Whether an entry belongs in the list shown. */
export function shows(r: ListRow, show: ListShow): boolean {
  if (show.startsWith("pos:")) return r.posOfSpeech.trim() === show.slice(4);
  if (show === "gloss") return r.glosses.length === 0 || r.glosses.some((g) => g.trim() === "");
  if (show === "category") return r.posOfSpeech.trim() === "";
  if (show === "unused") return !r.used;
  return true;
}

/** The categories (parts of speech) the entries use, the commonest first,
 * each with how many entries have it; `keep` stays listed (with 0) when the
 * list no longer has it, so a picked category does not vanish from its picker. */
export function categoriesOf(rows: readonly ListRow[], keep: ListShow | null = null): { category: string; count: number }[] {
  const n = new Map<string, number>();
  for (const r of rows) {
    const c = r.posOfSpeech.trim();
    if (c !== "") n.set(c, (n.get(c) ?? 0) + 1);
  }
  if (keep !== null && keep.startsWith("pos:") && !n.has(keep.slice(4))) n.set(keep.slice(4), 0);
  return [...n].map(([category, count]) => ({ category, count })).sort((a, b) => b.count - a.count || a.category.localeCompare(b.category));
}

/** The picker's label with how many entries it would list: "Needs a gloss (12)". */
export function showLabel(rows: readonly ListRow[], s: { id: ListShow; label: string }): string {
  return `${s.label} (${rows.filter((r) => shows(r, s.id)).length.toLocaleString("en")})`;
}

/** What a lexicon table import did (`lexicon_table::import`). */
export type TableImport = { entries: number; senses: number; unchanged: number; ambiguous: string[]; noLemma: number; columns: string[]; ignored: string[] };

/** The toast after a lexicon table import, in words. */
export function tableImportLines(r: TableImport): string[] {
  const n = (x: number, one: string, many: string) => `${x.toLocaleString("en")} ${x === 1 ? one : many}`;
  const lines = [
    r.entries === 0 && r.senses === 0
      ? "Nothing new: every row is in the lexicon already."
      : `Added ${n(r.entries, "entry", "entries")} and ${n(r.senses, "sense", "senses")}${r.unchanged > 0 ? `; ${n(r.unchanged, "row was", "rows were")} there already` : ""}.`,
    `Read ${r.columns.join(", ")}.`,
  ];
  if (r.ignored.length > 0) lines.push(`Left out ${r.ignored.length === 1 ? "the column" : "the columns"} ${r.ignored.map((c) => `«${c}»`).join(", ")}.`);
  if (r.ambiguous.length > 0) {
    const shown = r.ambiguous.slice(0, 5).map((l) => `«${l}»`).join(", ");
    lines.push(`Not added: ${shown}${r.ambiguous.length > 5 ? " and others" : ""} ${r.ambiguous.length === 1 ? "names" : "name"} several entries; give those rows a homograph column.`);
  }
  if (r.noLemma > 0) lines.push(`${n(r.noLemma, "row has", "rows have")} no word and ${r.noLemma === 1 ? "was" : "were"} skipped.`);
  return lines;
}
