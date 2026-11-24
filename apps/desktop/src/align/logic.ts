/** Pairing a text with another text that translates it (src-tauri/src/align.rs). */

export type Pair = { segment: string | null; source: string[]; translation: string[]; hasOne: boolean };
export type Alignment = { pairs: Pair[]; sources: number; translations: number; oneToOne: number; unmatched: number; already: number; written: number };

const n = (k: number, one: string, many: string) => `${k} ${k === 1 ? one : many}`;

/** Which shape a pair has, in words, or null for one sentence and one. */
export function pairNote(p: Pair): string | null {
  const [a, b] = [p.source.length, p.translation.length];
  if (a === 1 && b === 1) return null;
  if (a === 0) return "matched no sentence";
  if (b === 0) return "no translation found";
  if (a === 2 && b === 1) return "one translation for two sentences: it goes on the first";
  if (a === 1 && b === 2) return "two translation sentences joined";
  return `${a} sentences, ${b} translation sentences`;
}

/** How many translations a write would add. */
export function toWrite(a: Alignment): number {
  return a.pairs.filter((p) => p.segment !== null && !p.hasOne && p.translation.length > 0).length;
}

/** The preview's summary line. */
export function alignLine(a: Alignment): string {
  const parts = [`${n(a.sources, "sentence", "sentences")} and ${n(a.translations, "translation sentence", "translation sentences")}: ${a.oneToOne} paired one to one`];
  const other = a.pairs.filter((p) => pairNote(p) !== null && p.source.length > 0 && p.translation.length > 0).length;
  if (other > 0) parts.push(`${other} joined`);
  const alone = a.pairs.filter((p) => p.source.length > 0 && p.translation.length === 0).length;
  if (alone > 0) parts.push(`${n(alone, "sentence", "sentences")} with no translation found`);
  if (a.unmatched > 0) parts.push(`${n(a.unmatched, "translation sentence", "translation sentences")} matched nothing`);
  if (a.already > 0) parts.push(`${a.already} already translated and left as they are`);
  return parts.join("; ") + ".";
}

/** What a write did, for the toast. */
export function writtenLine(a: Alignment, translationTitle: string): string {
  if (a.written === 0) return "Every matched sentence already has a translation, so nothing was written.";
  return `Wrote ${n(a.written, "translation", "translations")} from «${translationTitle}». That text is unchanged; delete it if you no longer need it.`;
}
