// Sentences like this one, under the Gloss panel's translation: the project's
// sentences sharing most of this one's rarer words, with their translations
// (src-tauri/src/like_sentences.rs). Wording here, npm-tested.

/** A sentence like the one on screen. */
export type Like = { textId: string; textTitle: string; tierId: string; segmentId: string; content: string; translation: string | null; shared: string[]; score: number };

/** Remembered on this computer: whether the list is shown. */
export const LIKE_KEY = "wugs.gloss.like";

/** How alike, in words a person reads at a glance. */
export function likeness(score: number): string {
  if (score >= 0.999) return "the same words";
  if (score >= 0.6) return "very like it";
  if (score >= 0.35) return "like it";
  return "a little like it";
}

/** The note under the heading. */
export function likeNote(list: readonly Like[]): string {
  if (list.length === 0) return "No other sentence shares this one's rarer words yet.";
  const translated = list.filter((l) => l.translation !== null && l.translation.trim() !== "").length;
  const them = list.length === 1 ? "1 sentence shares" : `${list.length} sentences share`;
  const tr = translated === 0 ? "" : translated === list.length ? (list.length === 1 ? ", with its translation" : ", with their translations") : `, ${translated} with a translation`;
  return `${them} words with this one${tr}. Click one to open it.`;
}

/** The hover title of a row: what the two share. */
export function likeTitle(l: Like): string {
  const words = l.shared.length === 0 ? "" : ` Both have ${l.shared.map((w) => `«${w}»`).join(", ")}.`;
  return `${likeness(l.score)} (${Math.round(l.score * 100)}%) in «${l.textTitle || "(untitled)"}».${words}`;
}
