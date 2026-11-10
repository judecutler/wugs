// Leipzig Glossing Rules rendering decisions. Pure: no DOM.
// A gloss is a free string. `.` joins several meanings of one morph (a
// portmanteau: `1SG.NOM`, `dog.PL`) and stays inside one gloss. A component in
// capitals (letters and digits, at least one of them) is a grammatical
// category and is set in small capitals; anything else is a lexical gloss.

export type GlossPart = { text: string; smallCaps: boolean };

const GRAMMATICAL = /^[\p{Lu}\p{N}>]+$/u;

/** `PL`, `1SG`, `3`, `1>2` are grammatical; `dog`, `Ali`, `` are not. */
export const isGrammatical = (component: string): boolean => GRAMMATICAL.test(component) && /[\p{Lu}\p{N}]/u.test(component);

/** Every `.`-joined component is grammatical. An empty gloss is not. */
export function isAllCapsGloss(gloss: string): boolean {
  const g = gloss.trim();
  return g !== "" && g.split(".").every(isGrammatical);
}

/** The gloss cut into runs to render, the dots kept so a portmanteau reads as one unit. */
export function glossParts(gloss: string): GlossPart[] {
  const parts: GlossPart[] = [];
  gloss.split(".").forEach((component, i) => {
    if (i > 0) parts.push({ text: ".", smallCaps: false });
    if (component !== "") parts.push({ text: component, smallCaps: isGrammatical(component) });
  });
  return parts;
}
