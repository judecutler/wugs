// Glossing kept in an ELAN file's own tiers: the words in a toast. Pure: no DOM.

export type TierGlossing = { index: number; names: string[]; hasMorphs: boolean; hasGlosses: boolean; kept: string[]; dropped: string[]; timed: boolean };
export type ReadGlossing = { sentences: number; words: number; morphs: number; glossed: number; kept: number; stray: number };

const n = (x: number, one: string, many: string) => `${x.toLocaleString("en")} ${x === 1 ? one : many}`;
const quoted = (names: string[]) => {
  const q = names.map((x) => `«${x}»`);
  return q.length <= 1 ? q.join("") : `${q.slice(0, -1).join(", ")} and ${q[q.length - 1]}`;
};

/** What a set of tiers holds and what reading them does, in a sentence or three. */
export function tierGlossingLines(g: TierGlossing): string[] {
  const parts = ["words", ...(g.hasMorphs ? ["morphs"] : []), ...(g.hasGlosses ? ["glosses"] : [])];
  const what = parts.length === 1 ? parts[0] : `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}`;
  const lines = [`Its tiers ${quoted(g.names.slice(1))} hold the ${what} of «${g.names[0]}». Read as glossing, they show in the Gloss panel and the corpus tools, and an ELAN export writes them back as tiers of their own.`];
  if (g.timed) lines.push("The words keep their order but not their times.");
  if (g.kept.length > 0) lines.push(`${quoted(g.kept)} ${g.kept.length === 1 ? "is" : "are"} kept on each word or morph.`);
  if (g.dropped.length > 0) lines.push(`${quoted(g.dropped)} ${g.dropped.length === 1 ? "is" : "are"} left out.`);
  return lines;
}

/** What reading did. */
export function readGlossingLines(r: ReadGlossing): string[] {
  const lines = [`Read ${n(r.words, "word", "words")} in ${n(r.sentences, "sentence", "sentences")}${r.morphs > 0 ? `, with ${n(r.morphs, "morph", "morphs")} (${r.glossed.toLocaleString("en")} glossed)` : ""}.`];
  if (r.kept > 0) lines.push(`${n(r.kept, "sentence already had", "sentences already had")} words and ${r.kept === 1 ? "was" : "were"} left as ${r.kept === 1 ? "it was" : "they were"}.`);
  if (r.stray > 0) lines.push(`${n(r.stray, "annotation", "annotations")} belonged to no sentence and ${r.stray === 1 ? "was" : "were"} left out.`);
  return lines;
}
