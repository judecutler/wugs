// Concordance display (roadmap 71): keyword in the middle, its context either
// side, sortable by the words to the left or right. Pure: no DOM.

export type ConcordanceKind = "word" | "morph" | "gloss" | "translation";

export type Hit = {
  textId: string;
  textTitle: string;
  segmentId: string;
  tierId: string;
  startMs: number | null;
  endMs: number | null;
  words: string[];
  at: number;
  breakdown: string | null;
  glosses: string | null;
};

export const KINDS: { id: ConcordanceKind; label: string; hint: string }[] = [
  { id: "word", label: "Words", hint: "a word as transcribed: kani, ni*" },
  { id: "morph", label: "Morphs", hint: "a morph: kani, -ra" },
  { id: "gloss", label: "Glosses", hint: "a gloss: PL, 1SG, count" },
  { id: "translation", label: "Translations", hint: "a word of a free translation" },
];

export type SortBy = "text" | "left" | "right";

export const SORTS: { id: SortBy; label: string }[] = [
  { id: "text", label: "In text order" },
  { id: "left", label: "By the word before" },
  { id: "right", label: "By the word after" },
];

export const left = (h: Hit): string => h.words.slice(0, h.at).join(" ");
export const right = (h: Hit): string => h.words.slice(h.at + 1).join(" ");
export const keyword = (h: Hit): string => h.words[h.at] ?? "";

// Case is ignored, diacritics are not: ŋa and na are different words.
const collator = new Intl.Collator(undefined, { sensitivity: "accent", numeric: true });

/** Sorted copy. Left context sorts by the nearest word first, then the next
 * one out, as concordancers (AntConc's 1L, 2L) do; right context likewise. */
export function sortHits(hits: readonly Hit[], by: SortBy): Hit[] {
  if (by === "text") return [...hits];
  const keyOf = (h: Hit): string[] => (by === "left" ? h.words.slice(0, h.at).reverse() : h.words.slice(h.at + 1));
  return [...hits].sort((a, b) => {
    const ka = keyOf(a);
    const kb = keyOf(b);
    for (let i = 0; i < Math.max(ka.length, kb.length); i++) {
      const x = ka[i];
      const y = kb[i];
      if (x === undefined) return -1;
      if (y === undefined) return 1;
      const c = collator.compare(x, y);
      if (c !== 0) return c;
    }
    return 0;
  });
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
export function asTsv(hits: readonly Hit[]): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  return ["left\tkeyword\tright\ttext", ...hits.map((h) => [left(h), keyword(h), right(h), h.textTitle].map(clean).join("\t"))].join("\n");
}

/** Right-to-left scripts (Hebrew, Arabic, Syriac, Thaana, N'Ko …): the row is laid out from the right. */
export const isRtl = (s: string): boolean => /[\u0590-\u08FF\uFB1D-\uFDFF\uFE70-\uFEFF]/u.test(s);
