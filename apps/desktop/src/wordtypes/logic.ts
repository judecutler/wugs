// Gloss by word type: an analysis typed once for every unanalysed use of a
// word form. A breakdown as the Gloss panel reads it (`ni- kani`, `ni-kani`)
// and its glosses in the same order (`1SG count`, `1SG-count`). Pure: no DOM.

import { parseBreakdown } from "../gloss/morphs.ts";
import type { MorphSpec } from "../gloss/suggest.ts";

export type Typed = { ok: true; morphs: MorphSpec[] } | { ok: false; error: string };

/** The analysis typed for a word type. Glosses split at spaces and at the
 * Leipzig boundaries `-` `=` `~`; none at all leaves every morph unglossed. */
export function typedAnalysis(breakdown: string, glosses: string): Typed {
  const parsed = parseBreakdown(breakdown.trim());
  if (!parsed.ok) return parsed;
  if (parsed.morphs.length === 0) return { ok: false, error: "Type the word broken into its morphs, such as ni- kani." };
  const g = glosses
    .trim()
    .split(/[\s=~-]+/u)
    .filter((x) => x !== "");
  if (g.length > 0 && g.length !== parsed.morphs.length) {
    const m = parsed.morphs.length;
    return { ok: false, error: `${m === 1 ? "1 morph" : `${m} morphs`} but ${g.length === 1 ? "1 gloss" : `${g.length} glosses`}: give each morph one gloss, in order.` };
  }
  return { ok: true, morphs: parsed.morphs.map((m, i) => ({ surface: m.surface, type: m.type, gloss: g[i] ?? null, glossesByLang: {}, sense: null })) };
}

/** The line above the list. */
export function typesLine(rows: number, unbrokenSentences: number, unbrokenWords: number): string {
  const left = rows === 0 ? "Every word broken into words is analysed." : `${rows === 1 ? "1 word form has" : `${rows} word forms have`} uses not yet analysed, the most frequent first.`;
  if (unbrokenSentences === 0) return left;
  const s = unbrokenSentences === 1 ? "1 sentence" : `${unbrokenSentences} sentences`;
  return `${left} ${s} (${unbrokenWords} words) ${unbrokenSentences === 1 ? "is" : "are"} not broken into words yet, so ${unbrokenSentences === 1 ? "its" : "their"} words are not listed.`;
}

/** The toast after an analysis went to every use. */
export function appliedLine(form: string, applied: number, texts: number, linked = 0): string {
  if (applied === 0) return `No use of «${form}» was left to analyse.`;
  const link = linked === 0 ? "" : ` ${linked === 1 ? "1 morph is" : `${linked} morphs are`} linked to the lexicon entry with that spelling and gloss.`;
  return `Analysed ${applied === 1 ? "1 use" : `${applied} uses`} of «${form}» in ${texts === 1 ? "1 text" : `${texts} texts`}.${link}`;
}
