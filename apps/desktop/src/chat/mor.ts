// Words for reading a CHAT transcript's %mor and %gra tiers as tags and trees (src-tauri/src/chat_mor.rs). Pure.

export type MorRead = { withMor: number; withGra: number; read: number; kept: number; unaligned: number; words: number };

const sentences = (n: number) => (n === 1 ? "one sentence" : `${n} sentences`);

/** The offer after a CHAT import, or null when no sentence has a %mor tier. */
export function morOfferLines(r: MorRead): string[] | null {
  if (r.withMor === 0) return null;
  const gra = r.withGra === 0 ? "" : r.withGra === r.withMor ? " and %gra" : ` (${r.withGra} with %gra)`;
  return [`${sentences(r.withMor)[0]!.toUpperCase()}${sentences(r.withMor).slice(1)} ${r.withMor === 1 ? "carries" : "carry"} CLAN's %mor${gra}. Read them as lemmas, part-of-speech tags${r.withGra > 0 ? " and dependency trees" : ""}, and they show in the Sentence panel, CQP queries, lemma and tag lists and word sketches.`];
}

/** What reading did. */
export function morReadLines(r: MorRead): string[] {
  if (r.withMor === 0) return ["This text has no %mor tiers to read."];
  const lines = [r.read === 0 ? "No sentence was given tags." : `Read ${sentences(r.read)} (${r.words.toLocaleString("en")} words): lemmas and tags from %mor${r.withGra > 0 ? ", relations and heads from %gra as GRASP writes them" : ""}. They are marked as not yet checked in the Sentence panel.`];
  if (r.kept > 0) lines.push(`${sentences(r.kept)[0]!.toUpperCase()}${sentences(r.kept).slice(1)} already had an analysis and ${r.kept === 1 ? "was" : "were"} left alone.`);
  if (r.unaligned > 0) lines.push(`${sentences(r.unaligned)[0]!.toUpperCase()}${sentences(r.unaligned).slice(1)} did not line up word for word with ${r.unaligned === 1 ? "its" : "their"} %mor (a filler or a word MOR left out) and ${r.unaligned === 1 ? "was" : "were"} left alone.`);
  return lines;
}
