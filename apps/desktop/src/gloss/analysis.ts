// Turning a typed breakdown into an analysis without losing glossing work. Pure: no DOM.
import type { ParsedMorph } from "./morphs.ts";
import type { MorphSpec } from "./suggest.ts";

/** One MorphSpec per parsed morph. A morph that was already there (same surface and type, matched in
 * order) keeps its glosses in every language and its sense link, so re-editing a breakdown
 * only costs the morphs that really changed. */
export function mergeAnalysis(parsed: readonly ParsedMorph[], current: readonly MorphSpec[]): MorphSpec[] {
  const free = [...current];
  return parsed.map((p) => {
    const i = free.findIndex((c) => c.surface.normalize("NFC") === p.surface && c.type === p.type);
    if (i === -1) return { surface: p.surface, type: p.type, gloss: null, glossesByLang: {}, sense: null };
    const [kept] = free.splice(i, 1);
    return { ...kept!, glossesByLang: { ...kept!.glossesByLang } };
  });
}

/** The word's morphs as the engine takes them back: the same analysis without ids or display labels. */
export function analysisOf(word: { morphs: readonly MorphSpec[] }): MorphSpec[] {
  return word.morphs.map((m) => ({ surface: m.surface, type: m.type, gloss: m.gloss, glossesByLang: { ...m.glossesByLang }, sense: m.sense }));
}

/** What a typed breakdown does to a word. Emptying the field never clears an analysis by
 * itself, on blur or on Enter: that takes the explicit Clear analysis action. */
export function breakdownEffect(parsed: readonly ParsedMorph[], currentMorphs: number): "apply" | "keep" {
  return parsed.length === 0 && currentMorphs > 0 ? "keep" : "apply";
}
