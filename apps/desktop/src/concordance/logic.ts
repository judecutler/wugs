// Concordance display (roadmap 71): keyword in the middle, its context either
// side, sortable by the words to the left or right. Pure: no DOM.

export type ConcordanceKind = "word" | "morph" | "gloss" | "translation";
/** What the backend searches: the panel's kinds, or a sense id (a lexicon entry's occurrences). */
export type QueryKind = ConcordanceKind | "sense";
/** The Concordance panel's chips: the shared kinds and a CQP query (its own command). */
export type PanelKind = ConcordanceKind | "query";

export type Hit = {
  textId: string;
  textTitle: string;
  segmentId: string;
  tierId: string;
  startMs: number | null;
  endMs: number | null;
  words: string[];
  at: number;
  /** Words the match covers from `at` (a phrase or a CQP query); 1 when absent. */
  len?: number;
  breakdown: string | null;
  glosses: string | null;
  /** For a CQP query: each word's tag (UPOS, else XPOS; "" for none). */
  tags?: string[];
};

export const KINDS: { id: ConcordanceKind; label: string; hint: string }[] = [
  { id: "word", label: "Words", hint: "a word as transcribed: kani, ni*" },
  { id: "morph", label: "Morphs", hint: "a morph: kani, -ra" },
  { id: "gloss", label: "Glosses", hint: "a gloss: PL, 1SG, count" },
  { id: "translation", label: "Translations", hint: "a word of a free translation" },
];

/** What Frequencies counts: the shared kinds, and lemmas and tags (as a CQP query reads them). */
export type FreqKind = ConcordanceKind | "lemma" | "tag";

export const FREQ_KINDS: { id: FreqKind; label: string; hint: string }[] = [
  ...KINDS,
  { id: "lemma", label: "Lemmas", hint: "a lemma, from CoNLL-U or a glossed word's entry" },
  { id: "tag", label: "Tags", hint: "a part-of-speech tag (UPOS)" },
];

/** The Concordance's chips: Frequencies shares KINDS, so the CQP chip is the Concordance's own. */
export const PANEL_KINDS: { id: PanelKind; label: string; hint: string }[] = [...KINDS, { id: "query", label: "CQP query", hint: '[lemma="go"] [upos="ADP"], "ni.*" %c' }];

/** The reminder under the chips while a CQP query is chosen. */
export const QUERY_HELP =
  'Tokens in [ ]: word, lemma, upos, xpos, feats, gloss, deprel, headword or headlemma = "a regular expression" (whole value; %c ignores case, %d diacritics), joined with & | !. [] is any token; ? * + {1,3} repeat it. "dog" is [word="dog"]. End with :: match.text_genre="news" to search only texts with that detail.';

export type SortBy = "text" | "left" | "right" | "left2" | "right2" | "keyword";

export const SORTS: { id: SortBy; label: string }[] = [
  { id: "text", label: "In text order" },
  { id: "left", label: "By the word before" },
  { id: "right", label: "By the word after" },
  { id: "left2", label: "By the second word before" },
  { id: "right2", label: "By the second word after" },
  { id: "keyword", label: "By the match, then after" },
];

/** Each sort's keys as positions, AntConc's sort levels: −1 the word just
 * before the match, 1 the word just after, 0 the match itself; `null` the
 * rest of the context outward from the last key. */
const LEVELS: Record<Exclude<SortBy, "text">, number[]> = {
  left: [-1, -2, -3, -4, -5],
  right: [1, 2, 3, 4, 5],
  left2: [-2, -1, -3, -4],
  right2: [2, 1, 3, 4],
  keyword: [0, 1, 2, 3],
};

const span = (h: Hit): number => Math.max(1, h.len ?? 1);
export const left = (h: Hit): string => h.words.slice(0, h.at).join(" ");
export const right = (h: Hit): string => h.words.slice(h.at + span(h)).join(" ");
export const keyword = (h: Hit): string => h.words.slice(h.at, h.at + span(h)).join(" ");

// Case is ignored, diacritics are not: ŋa and na are different words.
const collator = new Intl.Collator(undefined, { sensitivity: "accent", numeric: true });

/** Sorted copy. Left context sorts by the nearest word first, then the next
 * one out, as concordancers (AntConc's 1L, 2L) do; right context likewise. */
export function sortHits(hits: readonly Hit[], by: SortBy): Hit[] {
  if (by === "text") return [...hits];
  const levels = LEVELS[by];
  // A word that is not there (the sentence starts or ends first) sorts before any word.
  const at = (h: Hit, pos: number): string | undefined => (pos === 0 ? keyword(h) : pos < 0 ? h.words[h.at + pos] : h.words[h.at + span(h) - 1 + pos]);
  const keyOf = (h: Hit): (string | undefined)[] => levels.map((pos) => at(h, pos));
  return [...hits].sort((a, b) => {
    const ka = keyOf(a);
    const kb = keyOf(b);
    for (let i = 0; i < Math.max(ka.length, kb.length); i++) {
      const x = ka[i];
      const y = kb[i];
      if (x === undefined && y === undefined) continue;
      if (x === undefined) return -1;
      if (y === undefined) return 1;
      const c = collator.compare(x, y);
      if (c !== 0) return c;
    }
    return 0;
  });
}

/** The context word a sort orders by first (−1 just before, 2 the second after), to mark in the lines; null when none. */
export const sortedWord = (by: SortBy): number | null => (by === "text" || by === "keyword" ? null : LEVELS[by][0]!);

/** A shown context cut around the word `pos` counts from the match
 * (−1 the last word of the left context, 1 the first of the right), so the
 * line can mark it: [before, word, after]; the word is "" when the context
 * is too short to hold it. */
export function markWord(context: string, side: "left" | "right", pos: number): [string, string, string] {
  const words = context.split(" ");
  const i = side === "left" ? words.length + pos : pos - 1;
  if (context === "" || i < 0 || i >= words.length || words[i] === "…" || words[i]!.startsWith("…")) return [context, "", ""];
  const before = words.slice(0, i).join(" ");
  const after = words.slice(i + 1).join(" ");
  return [before === "" ? "" : `${before} `, words[i]!, after === "" ? "" : ` ${after}`];
}

/** At most `chars` characters of context, cut at a word and marked with … on the far side. */
export function clip(context: string, chars: number, side: "left" | "right"): string {
  if (context.length <= chars) return context;
  if (side === "left") {
    const cut = context.slice(context.length - chars);
    const space = cut.indexOf(" ");
    return `…${space >= 0 ? cut.slice(space) : cut}`;
  }
  const cut = context.slice(0, chars);
  const space = cut.lastIndexOf(" ");
  return `${space > 0 ? cut.slice(0, space) : cut} …`;
}

/** The count line: "12 in 3 texts", or how many are shown of how many found. */
export function countLine(shown: number, total: number, texts: number): string {
  if (total === 0) return "Nothing found.";
  const where = texts === 1 ? "1 text" : `${texts} texts`;
  if (shown < total) return `${total} found in ${where}; the first ${shown} are shown.`;
  return `${total} ${total === 1 ? "occurrence" : "occurrences"} in ${where}.`;
}

/** The hits as tab-separated rows (left, keyword, right, text), for a spreadsheet. */
/** Each line's speaker: the name of its track, when the lines come from more
 * than one track (a CHAT conversation); else null, as there is nothing to tell apart. */
export function speakerOf(hits: readonly Hit[], names: ReadonlyMap<string, string>): ((h: Hit) => string) | null {
  if (new Set(hits.map((h) => h.tierId)).size < 2) return null;
  return (h) => names.get(h.tierId) ?? "";
}

export function asTsv(hits: readonly Hit[], details?: ReadonlyMap<string, ReadonlyMap<string, string>>, under?: (h: Hit) => string | null, speaker?: ((h: Hit) => string) | null, category?: ((h: Hit) => string) | null): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  if (details === undefined) return ["left\tkeyword\tright\ttext", ...hits.map((h) => [left(h), keyword(h), right(h), h.textTitle].map(clean).join("\t"))].join("\n");
  // The fuller table, one line per hit with what a statistical analysis of
  // the lines needs: where it is, its analysis, and its text's details.
  const analysed = hits.some((h) => h.breakdown !== null || h.glosses !== null);
  const keys = [...new Set(hits.flatMap((h) => [...(details.get(h.textId)?.keys() ?? [])]))].sort();
  const secs = (ms: number | null) => (ms === null ? "" : (ms / 1000).toFixed(3));
  // The line under each hit (its translation, or the sentence a translation translates), when shown.
  const translated = under !== undefined && hits.some((h) => (under(h) ?? "") !== "");
  const head = ["left", "keyword", "right", "text", ...(speaker ? ["speaker"] : []), ...(category ? ["category"] : []), "start_s", "end_s", ...(analysed ? ["breakdown", "glosses"] : []), ...(translated ? ["translation"] : []), ...keys];
  const rows = hits.map((h) => {
    const d = details.get(h.textId);
    return [left(h), keyword(h), right(h), h.textTitle, ...(speaker ? [speaker(h)] : []), ...(category ? [category(h)] : []), secs(h.startMs), secs(h.endMs), ...(analysed ? [h.breakdown ?? "", h.glosses ?? ""] : []), ...(translated ? [under?.(h) ?? ""] : []), ...keys.map((k) => d?.get(k) ?? "")].map(clean).join("\t");
  });
  return [head.map(clean).join("\t"), ...rows].join("\n");
}

const escHtml = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** The lines as an HTML table a word processor takes as a KWIC display:
 * the left context right-aligned and clipped to `chars`, the match in bold,
 * the right context clipped, the text's title small; a spreadsheet takes
 * the same table cell by cell. */
export function kwicHtml(hits: readonly Hit[], chars = 40, under?: (h: Hit) => string | null): string {
  const cell = "padding:1px 6px;vertical-align:baseline;white-space:nowrap";
  const rows = hits.map((h) => {
    const line = `<tr><td style="${cell};text-align:right">${escHtml(clip(left(h), chars, "left"))}</td><td style="${cell}"><b>${escHtml(keyword(h))}</b></td><td style="${cell}">${escHtml(clip(right(h), chars, "right"))}</td><td style="${cell};font-size:80%;color:#555">${escHtml(h.textTitle)}</td></tr>`;
    const t = under?.(h) ?? "";
    // The translation in italics under its line, as a handout prints it.
    return t === "" ? line : `${line}<tr><td></td><td colspan="2" style="padding:0 6px 4px;font-style:italic;font-size:90%">‘${escHtml(t)}’</td><td></td></tr>`;
  });
  return `<table style="border-collapse:collapse;font-family:'Charis SIL','Doulos SIL',serif">${rows.join("")}</table>`;
}

/** Whether any line carries a word's tag (a CQP query over a tagged corpus). */
export const hasTags = (hits: readonly Hit[]): boolean => hits.some((h) => h.tags?.some((t) => t !== "") ?? false);

/** The line with each word written word_TAG, as AntConc shows a tagged corpus; a word with no tag stays as it is. */
export function withTags(h: Hit): Hit {
  if (h.tags === undefined) return h;
  return { ...h, words: h.words.map((w, i) => (h.tags![i] ? `${w}_${h.tags![i]}` : w)) };
}

/** Right-to-left scripts (Hebrew, Arabic, Syriac, Thaana, N'Ko …): the row is laid out from the right. */
export const isRtl = (s: string): boolean => /[\u0590-\u08FF\uFB1D-\uFDFF\uFE70-\uFEFF]/u.test(s);

/** Sample sizes a concordance can be thinned to. */
export const THIN_SIZES = [50, 100, 250, 500] as const;

/** A random sample of `n` lines that the same `seed` always gives again
 * (CQPweb's and Sketch Engine's "thin"), kept in the order found. */
export function thin(hits: readonly Hit[], n: number, seed: string): Hit[] {
  if (n <= 0 || hits.length <= n) return [...hits];
  // FNV-1a of the seed, then mulberry32.
  let s = 0x811c9dc5;
  for (let i = 0; i < seed.length; i++) s = Math.imul(s ^ seed.charCodeAt(i), 0x01000193);
  const next = () => {
    s = (s + 0x6d2b79f5) | 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const idx = hits.map((_, i) => i);
  for (let i = 0; i < n; i++) {
    const j = i + Math.floor(next() * (idx.length - i));
    [idx[i], idx[j]] = [idx[j]!, idx[i]!];
  }
  return idx
    .slice(0, n)
    .sort((a, b) => a - b)
    .map((i) => hits[i]!);
}

/** Said after the count line when the lines are thinned. */
export function thinLine(shown: number, read: number, total: number): string {
  const of = read < total ? `the first ${read.toLocaleString()} found` : `${read.toLocaleString()} lines`;
  return ` Thinned to ${shown.toLocaleString()} at random of ${of}; the same query gives the same ${shown.toLocaleString()}.`;
}

/** A search kept with the project (src-tauri/src/saved_searches.rs). */
export type SavedSearch = { name: string; kind: string; query: string; textId: string | null; textTitle: string | null; exact: boolean; within: import("../metadata/logic.ts").Where | null };

/** What a saved search looks for, in words, for its title in the list. */
export function savedLine(s: SavedSearch): string {
  const what = s.kind === "query" ? `the CQP query ${s.query}` : `${s.kind === "word" ? "words" : s.kind === "morph" ? "morphs" : s.kind === "gloss" ? "glosses" : "translation words"} matching «${s.query}»${s.exact ? " exactly" : ""}`;
  const where = s.textTitle ? ` in «${s.textTitle}»` : s.within ? (s.within.tier ? " on one track" : ` in texts whose ${s.within.key} is «${s.within.value}»`) : "";
  return `${what}${where}`;
}
