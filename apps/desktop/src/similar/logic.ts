// Similar words: the words used in the same contexts as a word (src-tauri/src/similar.rs).
// Display rules only; pure, no DOM.

export type SimilarRow = { item: string; similarity: number; freq: number; shared: string[] };
export type Similar = { rows: SimilarRow[]; word: string; freq: number; tokens: number; compared: number };

export const WINDOWS = [1, 2, 3, 5];
export const MINIMUMS = [2, 3, 5, 10, 20];
/** Below this many running words the vectors are too thin to say much. */
export const THIN = 5000;

const n = (x: number) => x.toLocaleString("en");

/** What was found, in words. */
export function similarNote(r: Similar, asked: string, window: number): string {
  if (r.freq === 0) return `«${asked}» does not occur here.`;
  const base = `«${r.word}» occurs ${n(r.freq)} ${r.freq === 1 ? "time" : "times"} in ${n(r.tokens)} running words; ${n(r.compared)} words were compared by the words within ${window} of them.`;
  const found = r.rows.length === 0 ? " No word shares its contexts." : ` The ${r.rows.length === 1 ? "word" : `${r.rows.length} words`} used most like it first.`;
  const thin = r.tokens < THIN ? ` With under ${n(THIN)} running words, read these as hints: similarity needs many contexts.` : "";
  return base + found + thin;
}

/** A similarity as a percentage-free two-decimal score. */
export const similarityCell = (s: number): string => s.toFixed(2);

/** The list as a tab-separated table. */
export function similarTsv(r: Similar): string {
  const lines = ["word\tsimilarity\tfrequency\tshared contexts"];
  for (const row of r.rows) lines.push([row.item, similarityCell(row.similarity), String(row.freq), row.shared.join(", ")].join("\t"));
  return lines.join("\n") + "\n";
}

/** A bar's width for a similarity, 0–100. */
export const barWidth = (s: number): number => Math.max(0, Math.min(100, Math.round(s * 100)));
