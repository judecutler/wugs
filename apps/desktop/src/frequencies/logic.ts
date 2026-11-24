// Frequency list (Scrolls): display rules. Pure: no DOM.

/** `lexicon` (single transcription words only): `entry` when an entry or one
 * of its forms is spelled this way, `analysed` when not but some use is broken
 * into morphs, `none` when the dictionary does not have it yet. */
export type LexiconStatus = "entry" | "analysed" | "none";
/** `dp`: Gries's DP, normalised: 0 spread over the texts in step with their
 * sizes, 1 all in one text; null with fewer than two texts to compare. */
export type FrequencyRow = { item: string; count: number; texts: number; lexicon?: LexiconStatus; dp?: number | null };
/** A row with its frequency rank (1 = most frequent), kept through sorting and filtering. */
export type RankedRow = FrequencyRow & { rank: number };

/** Ranks in the backend's order (most frequent first, ties as first met). */
export const ranked = (rows: readonly FrequencyRow[]): RankedRow[] => rows.map((r, i) => ({ ...r, rank: i + 1 }));
/** `words` is the running word count rates are taken from; `once` the sequences met once and left out (n-grams). */
export type Frequencies = { rows: FrequencyRow[]; tokens: number; types: number; words?: number; once?: number; unlabelled?: number };

/** A value as a CQP string matching exactly it (`cqp::literal`): regular-expression characters and quotes escaped. */
export const cqpLiteral = (value: string): string => value.replace(/[\\.+*?()|[\]{}^$#&\-~]/gu, (c) => `\\${c}`).replace(/"/gu, '\\"');

/** The CQP query whose lines a lemma or tag row counts, limited to the texts a detail picks. */
export function cqpFor(kind: "lemma" | "tag", item: string, within: { key: string; value: string; speaker?: boolean } | null): string {
  const q = `[${kind === "lemma" ? "lemma" : "upos"}="${cqpLiteral(item)}"]`;
  // A track or a speaker's detail picks no texts, and CQP has no word for tracks: the query reads every track.
  return within === null || within.key === "" || within.speaker === true ? q : `${q} :: match.text_${within.key}="${cqpLiteral(within.value)}"`;
}

/** The note under a lemma or tag list: words not yet lemmatised or tagged. */
export function unlabelledNote(f: Frequencies, kind: "lemma" | "tag"): string {
  const n = f.unlabelled ?? 0;
  if (n === 0) return "";
  const what = kind === "lemma" ? "lemma" : "tag";
  const how = kind === "lemma" ? "gloss the word and link its root to a lexicon entry" : "gloss the word and give its root's lexicon entry a category";
  return ` ${n.toLocaleString("en")} ${n === 1 ? "word has" : "words have"} no ${what} yet: tag and parse its text in the Sentence panel, or ${how}.`;
}

export type FreqSort = "count" | "alpha" | "ending" | "texts" | "spread" | "adjusted";

export const FREQ_SORTS: { id: FreqSort; label: string }[] = [
  { id: "count", label: "Most frequent first" },
  { id: "alpha", label: "Alphabetical" },
  { id: "ending", label: "By ending (reverse list)" },
  { id: "texts", label: "In most texts first" },
  { id: "spread", label: "Most evenly spread first" },
  { id: "adjusted", label: "By frequency adjusted for spread" },
];

/** Whether any row has a spread to show (two texts or more were counted). */
export const hasSpread = (rows: readonly FrequencyRow[]): boolean => rows.some((r) => typeof r.dp === "number");

/** Frequency adjusted for dispersion: the count times 1 − DP, so a word spread over every text keeps its count and one crowded into a single text loses it (the measure frequency dictionaries rank by); null without a DP. */
export const adjusted = (r: FrequencyRow): number | null => (typeof r.dp === "number" ? r.count * (1 - r.dp) : null);

/** The adjusted frequency as the table prints it. */
export const adjustedCell = (r: FrequencyRow): string => {
  const a = adjusted(r);
  return a === null ? "–" : a.toFixed(1);
};

/** A row's DP as the table prints it. */
export const dpCell = (dp: number | null | undefined): string => (typeof dp === "number" ? dp.toFixed(2) : "");

/** The sentence under a list with a DP column, saying how to read it. */
export const spreadNote = (rows: readonly FrequencyRow[]): string =>
  hasSpread(rows) ? " DP says how evenly each is spread over the texts: 0 in step with each text's size, 1 all in one text." : "";

const collator = new Intl.Collator(undefined, { sensitivity: "accent", numeric: true });
const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });

/** The item read from its end: its letters (each with its marks) in reverse,
 * so words sharing an ending sort together, as a retrograde dictionary
 * lists them: `kitaplar` beside `adamlar`, and every `-lar` together. */
export function reversed(item: string): string {
  return Array.from(graphemes.segment(item.normalize("NFC")), (g) => g.segment)
    .reverse()
    .join("");
}

/** Sorted copy; ties keep the backend's order (most frequent, then first met). */
export function sortRows<R extends FrequencyRow>(rows: readonly R[], by: FreqSort): R[] {
  if (by === "count") return [...rows];
  if (by === "texts") return [...rows].sort((a, b) => b.texts - a.texts || b.count - a.count);
  // The core vocabulary first: evenly spread, then frequent; rows with no DP last.
  if (by === "spread") return [...rows].sort((a, b) => (a.dp ?? 2) - (b.dp ?? 2) || b.count - a.count);
  if (by === "adjusted") return [...rows].sort((a, b) => (adjusted(b) ?? -1) - (adjusted(a) ?? -1) || b.count - a.count);
  if (by === "ending") {
    const keyed = rows.map((r) => ({ r, k: reversed(r.item) }));
    return keyed.sort((a, b) => collator.compare(a.k, b.k) || collator.compare(a.r.item, b.r.item)).map((x) => x.r);
  }
  return [...rows].sort((a, b) => collator.compare(a.item, b.item));
}

/** Occurrences per thousand, as corpus tools print it: one decimal under 10, none above. */
export function perThousand(count: number, tokens: number): string {
  if (tokens === 0) return "0";
  const v = (count * 1000) / tokens;
  return v >= 10 ? Math.round(v).toLocaleString("en") : v.toFixed(1);
}

/** Rows whose item contains the filter, ignoring case but not diacritics. */
export function filterRows<R extends FrequencyRow>(rows: readonly R[], filter: string): R[] {
  const f = filter.trim();
  if (f === "") return [...rows];
  const lower = f.toLocaleLowerCase();
  return rows.filter((r) => r.item.toLocaleLowerCase().includes(lower));
}

/** How many texts a row must be met in to be listed. */
export const TEXT_RANGES: readonly { min: number; label: string }[] = [
  { min: 1, label: "In any text" },
  { min: 2, label: "In 2+ texts" },
  { min: 3, label: "In 3+ texts" },
  { min: 5, label: "In 5+ texts" },
  { min: 10, label: "In 10+ texts" },
];

/** Only the rows met in at least `min` texts: words a corpus shares rather than one text's names and topics; with 3- or 4-word sequences, Biber's lexical bundles. */
export const inTexts = <R extends FrequencyRow>(rows: readonly R[], min: number): R[] => (min <= 1 ? [...rows] : rows.filter((r) => r.texts >= min));

/** The note's words for the range: how many rows it keeps. */
export function rangeNote(kept: number, min: number, size: number): string {
  if (min <= 1) return "";
  const bundles = size >= 3 ? " (lexical bundles, as Biber counts them)" : "";
  return ` ${kept.toLocaleString("en")} ${kept === 1 ? "is" : "are"} met in ${min} texts or more${bundles}.`;
}

/** Only the words the lexicon does not have yet (no entry or form spelled so). */
export const notInLexicon = <R extends FrequencyRow>(rows: readonly R[]): R[] => rows.filter((r) => r.lexicon === "none" || r.lexicon === "analysed");

/** A row's lexicon column in words. */
export function lexiconCell(status: LexiconStatus | undefined): string {
  if (status === "entry") return "Yes";
  if (status === "analysed") return "Glossed only";
  return status === "none" ? "No" : "";
}

/** The sentence under a word list about what the lexicon lacks:
 * "41 of these 318 words are not in the lexicon yet (they are used 96 times)." */
/** The most words Add all sends at once (the backend's MAX_WORDS). */
export const ADD_AT_ONCE = 5000;

/** The Add all button's words: the shown rows not in the lexicon, at most ADD_AT_ONCE. */
export function addLabel(missing: number, armed: boolean): string {
  const n = Math.min(missing, ADD_AT_ONCE);
  const words = n === 1 ? "1 word" : `${n.toLocaleString("en")} words`;
  return armed ? `Add ${words} as entries? Click again` : `Add the ${words} not in the lexicon`;
}

/** What Add all did, for the toast. */
export function addedLine(added: number, asked: number): string {
  if (added === 0) return "Every word was in the lexicon already.";
  const made = added === 1 ? "1 entry" : `${added.toLocaleString("en")} entries`;
  return `Made ${made}, each with an empty sense to gloss in the Lexicon panel (Needs a gloss lists them).${asked > added ? ` ${asked - added} were in the lexicon already or repeated a word.` : ""}`;
}

export function lexiconNote(rows: readonly FrequencyRow[]): string {
  const marked = rows.filter((r) => r.lexicon !== undefined);
  if (marked.length === 0) return "";
  const missing = notInLexicon(marked);
  if (missing.length === 0) return " Every word listed is in the lexicon.";
  const uses = missing.reduce((n, r) => n + r.count, 0);
  const n = (x: number) => x.toLocaleString("en");
  const used = `(used ${n(uses)} ${uses === 1 ? "time" : "times"})`;
  if (marked.length === 1) return ` This word is not in the lexicon yet ${used}.`;
  return ` ${n(missing.length)} of these ${n(marked.length)} words ${missing.length === 1 ? "is" : "are"} not in the lexicon yet ${used}.`;
}

/** "1,204 words, 318 different" in the kind's own nouns. */
export function summary(f: Frequencies, noun: { one: string; many: string }): string {
  if (f.tokens === 0) return `No ${noun.many} yet.`;
  const n = (x: number) => x.toLocaleString("en");
  const shown = f.rows.length < f.types - (f.once ?? 0) ? ` The ${n(f.rows.length)} most frequent are listed.` : "";
  const once = (f.once ?? 0) > 0 ? ` The ${n(f.once!)} met only once are left out.` : "";
  return `${n(f.tokens)} ${f.tokens === 1 ? noun.one : noun.many}, ${n(f.types)} different.${once}${shown}`;
}

/** Tab-separated, with a header row, for a spreadsheet; rank is the frequency rank, whatever the order. */
export function asTsv(rows: readonly RankedRow[], tokens: number, item = "item"): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  const lexicon = rows.some((r) => r.lexicon !== undefined);
  const spread = hasSpread(rows);
  const head = `rank\t${clean(item)}\tcount\tper 1000 words\ttexts${spread ? "\tDP\tadjusted frequency" : ""}${lexicon ? "\tin lexicon" : ""}`;
  return [head, ...rows.map((r) => [String(r.rank), clean(r.item), String(r.count), perThousand(r.count, tokens), String(r.texts), ...(spread ? [dpCell(r.dp), adjustedCell(r)] : []), ...(lexicon ? [lexiconCell(r.lexicon)] : [])].join("\t"))].join("\n");
}

/** Lexical richness of a word list: type/token ratio, Guiraud's index
 * (types over the square root of tokens, steadier across text lengths) and
 * the words met once (hapax legomena), when every type is listed. */
export function richness(f: Frequencies): string {
  if (f.tokens === 0 || f.types === 0) return "";
  const ttr = f.types / f.tokens;
  const guiraud = f.types / Math.sqrt(f.tokens);
  const head = ` Type/token ratio ${ttr.toFixed(2)} (Guiraud's index ${guiraud.toFixed(1)}).`;
  if (f.rows.length < f.types) return head;
  const once = f.rows.filter((r) => r.count === 1).length;
  if (once === 0) return head;
  const share = Math.round((once / f.types) * 100);
  return `${head} ${once.toLocaleString("en")} ${once === 1 ? "is" : "are"} met only once (hapax legomena, ${share}% of the different ones).`;
}
