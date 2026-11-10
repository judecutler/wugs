// How analysis suggestions are ranked, shown and offered. Pure: no DOM.
import { joinedBreakdown } from "./morphs.ts";
import type { MorphType } from "./morphs.ts";

export type MorphSpec = {
  surface: string;
  type: string;
  gloss: string | null;
  glossesByLang: Record<string, string>;
  sense: string | null;
};
export type Suggestion = { morphs: MorphSpec[]; count: number };

export type SuggestionLine = {
  /** `köpek-ler` */
  breakdown: string;
  /** `dog-PL`, with `?` where a morph has no gloss yet. */
  glosses: string;
  uses: number;
  /** Share of all earlier analyses of this form, 0 to 1. */
  share: number;
  usesLabel: string;
};

const KNOWN: readonly string[] = ["root", "prefix", "suffix", "infix", "clitic", "reduplication"];
const asType = (t: string): MorphType => (KNOWN.includes(t) ? (t as MorphType) : "root");

/** Highest count first; equal counts keep the order given, so the engine's tie order shows through. */
export function ranked(list: readonly Suggestion[]): Suggestion[] {
  return list.map((s, i) => ({ s, i })).sort((a, b) => b.s.count - a.s.count || a.i - b.i).map(({ s }) => s);
}

export function describe(s: Suggestion, all: readonly Suggestion[]): SuggestionLine {
  const total = all.reduce((n, x) => n + x.count, 0);
  return {
    breakdown: joinedBreakdown(s.morphs.map((m) => ({ surface: m.surface, type: asType(m.type) }))),
    glosses: s.morphs.map((m) => m.gloss ?? "?").join("-"),
    uses: s.count,
    share: total === 0 ? 0 : s.count / total,
    usesLabel: s.count === 1 ? "used once" : all.length > 1 ? `used ${s.count} of ${total} times` : `used ${s.count} times`,
  };
}

/** The suggestions to show for a word, best first, at most `limit`. A word that already has an analysis shows none. */
export function suggestionsToShow(list: readonly Suggestion[], analysed: boolean, limit = 3): Suggestion[] {
  return analysed ? [] : ranked(list).slice(0, limit);
}

/** The ghost chip is the top suggestion of an unanalysed word. */
export const ghostFor = (list: readonly Suggestion[], analysed: boolean): Suggestion | null => suggestionsToShow(list, analysed, 1)[0] ?? null;

/** The "apply to the others" link, or null when there is nothing to apply it to. The count comes first. */
export function applyOthersLabel(form: string, others: number, armed = false): string | null {
  if (others <= 0) return null;
  return armed ? `Give all ${others} «${form}» this analysis? Click again` : `Apply to ${others} other unanalysed «${form}»`;
}

/** Above this many other words, "apply to the others" takes a second click. */
export const CONFIRM_APPLY_ABOVE = 20;

export const applyNeedsConfirm = (others: number): boolean => others > CONFIRM_APPLY_ABOVE;
