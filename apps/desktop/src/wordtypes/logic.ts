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

export type Meaning = { word: string; together: number; of: number; dice: number };

/** The fields after a likely meaning is picked: with nothing typed, the form
 * as one root glossed with it; else the root's gloss set to it, the other
 * morphs' glosses kept (or ? where none was typed). */
export function withMeaning(form: string, breakdown: string, glosses: string, meaning: string): { breakdown: string; glosses: string } {
  const gloss = meaning.trim().replace(/[\s=~-]+/gu, ".");
  if (breakdown.trim() === "") return { breakdown: form, glosses: gloss };
  const parsed = parseBreakdown(breakdown.trim());
  if (!parsed.ok || parsed.morphs.length === 0) return { breakdown, glosses: gloss };
  const typed = glosses.trim().split(/[\s=~-]+/u).filter((x) => x !== "");
  const root = Math.max(0, parsed.morphs.findIndex((m) => m.type === "root"));
  const out = parsed.morphs.map((_, i) => (i === root ? gloss : typed.length === parsed.morphs.length ? typed[i] : "?"));
  return { breakdown, glosses: out.join(" ") };
}

/** A likely meaning's title: how often it stands with the word. */
export const meaningTitle = (form: string, m: Meaning): string =>
  `In the translations of ${m.together} of the ${m.of} translated sentences with «${form}» (Dice ${m.dice.toFixed(2)}). Click to gloss the root with it.`;

/** The rows whose suggestion is an earlier analysis of the same form (not a guess): safe to take for every use. */
export function earlierRows<R extends { suggestion: { guessed?: boolean } | null }>(rows: readonly R[]): R[] {
  return rows.filter((r) => r.suggestion !== null && r.suggestion.guessed !== true);
}

/** The rows whose guess is the word's own lexicon entry beyond doubt: one
 * root spelled as the word, the only entry spelled so, with one sense. */
export function headwordRows<R extends { headword?: boolean; suggestion: unknown }>(rows: readonly R[]): R[] {
  return rows.filter((r) => r.headword === true && r.suggestion !== null);
}

/** The headword button: how many forms and uses, and the second click it asks for. */
export function headwordLabel(forms: number, uses: number, armed: boolean): string {
  const f = forms === 1 ? "1 word form" : `${forms} word forms`;
  return armed ? `Gloss ${uses === 1 ? "1 use" : `${uses} uses`} of ${f} from the lexicon? Click again` : forms === 1 ? "Gloss 1 word form that is a lexicon headword" : `Gloss ${f} that are lexicon headwords`;
}

/** The Apply all button: how many forms and uses, and the second click it asks for. */
export function applyAllLabel(forms: number, uses: number, armed: boolean): string {
  const f = forms === 1 ? "1 word form" : `${forms} word forms`;
  return armed ? `Analyse ${uses === 1 ? "1 use" : `${uses} uses`} of ${f}? Click again` : `Use the earlier analysis for ${f}`;
}

/** The toast after Apply all. */
export function appliedAllLine(forms: number, uses: number, failed: number): string {
  const done = `Analysed ${uses === 1 ? "1 use" : `${uses} uses`} of ${forms === 1 ? "1 word form" : `${forms} word forms`}, each as it was analysed before.`;
  return failed === 0 ? `${done} Check them in the Interlinear tab.` : `${done} ${failed === 1 ? "1 form" : `${failed} forms`} could not be analysed; try them one at a time.`;
}
