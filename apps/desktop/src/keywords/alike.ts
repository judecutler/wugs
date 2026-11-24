// How alike the texts are (Cosine Delta over the project's most frequent
// words, src-tauri/src/stylometry.rs): the texts nearest one text, and the
// whole table to paste. Pure: no DOM.

export type Alike = { texts: { id: string; title: string; words: number }[]; mfw: number; similarity: number[][]; omitted: number };
export type Near = { id: string; title: string; words: number; similarity: number };

/** Texts under this many words give unsteady numbers. */
export const SHORT = 500;

/** The other texts, most like `id` first; none when it is not compared. */
export function nearest(a: Alike, id: string): Near[] {
  const i = a.texts.findIndex((t) => t.id === id);
  if (i < 0 || a.similarity.length === 0) return [];
  return a.texts
    .map((t, j) => ({ ...t, similarity: a.similarity[i]?.[j] ?? 0 }))
    .filter((_, j) => j !== i)
    .sort((x, y) => y.similarity - x.similarity || (x.title < y.title ? -1 : 1));
}

/** The line above the list. */
export function alikeLine(a: Alike, id: string): string {
  if (a.texts.length < 3) return "Comparing texts needs three texts or more with words.";
  const here = a.texts.find((t) => t.id === id);
  if (here === undefined) return "This text has no words to compare.";
  const short = a.texts.filter((t) => t.words < SHORT).length;
  return [
    `Compared by Cosine Delta over the project's ${a.mfw} most frequent words: 1 uses them the way this text does, 0 is unrelated, below 0 the opposite.`,
    short > 0 ? `${short === 1 ? "1 text has" : `${short} texts have`} under ${SHORT} words, so ${short === 1 ? "its" : "their"} numbers are unsteady.` : "",
    a.omitted > 0 ? `The ${a.omitted} shortest ${a.omitted === 1 ? "text was" : "texts were"} left out.` : "",
  ]
    .filter((s) => s !== "")
    .join(" ");
}

/** A similarity as a bar's share of its half: 0 to 1 either side of zero. */
export const barShare = (s: number): number => Math.min(1, Math.abs(s));

/** Every text against every other, as a table to paste. */
export function alikeTsv(a: Alike): string {
  const cell = (s: string) => s.replace(/[\t\r\n]+/g, " ");
  return [["", ...a.texts.map((t) => cell(t.title))].join("\t"), ...a.texts.map((t, i) => [cell(t.title), ...(a.similarity[i] ?? []).map((x) => x.toFixed(3))].join("\t"))].join("\n") + "\n";
}
