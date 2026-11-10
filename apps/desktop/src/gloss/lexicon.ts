// When glossing offers to grow the lexicon. Pure: no DOM.
import { isAllCapsGloss } from "./leipzig.ts";
import { ZERO } from "./morphs.ts";

export type OfferInput = { surface: string; type: string; gloss: string; sense: string | null; matches: number };

/** Offer "Add «surface» 'gloss' to the lexicon" for a root that has a gloss but neither a sense link nor
 * a lexicon match. Affixes, clitics, zero morphs and all-caps grammatical glosses never offer it. */
export function shouldOfferLexeme(m: OfferInput): boolean {
  const gloss = m.gloss.trim();
  return m.type === "root" && m.sense === null && m.matches === 0 && gloss !== "" && !isAllCapsGloss(gloss) && m.surface !== "" && m.surface !== ZERO;
}

export const offerText = (surface: string, gloss: string): string => `Add «${surface}» '${gloss.trim()}' to the lexicon`;

/** A match row in the autocomplete: lemma and sense gloss, with the language of the gloss when it is shown. */
export const matchLabel = (m: { lemma: string; gloss: string }): string => (m.gloss === "" ? m.lemma : `${m.lemma} – ${m.gloss}`);
