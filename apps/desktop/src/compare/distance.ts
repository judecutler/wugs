// How far each loaded language is from the project's, in sounds, as the
// ASJP project measures it (Wichmann et al. 2010): LDN, the mean edit
// distance over sounds between the two words for the same concept (as a
// share of the longer), and LDND, that mean divided by the mean distance
// between words for different concepts, which takes out the likeness two
// languages' sound systems give any two words. LDND near 1 or above is no
// closer than chance; well below 1, the words for the same things are
// alike. Pure, npm-tested.
import { similarity, type Row } from "./logic.ts";

export type Distance = { language: string; compared: number; ldn: number; ldnd: number | null };

/** Concepts read for the different-concept baseline. */
export const BASELINE_CONCEPTS = 200;
/** Concepts compared below which the numbers are not given. */
export const MIN_COMPARED = 10;

/** The closest distance between any of two concepts' forms. */
const closest = (a: readonly string[], b: readonly string[]): number => Math.min(...a.flatMap((x) => b.map((y) => 1 - similarity(x, y))));

/** Each language's LDN and LDND against the project's answers, closest first. */
export function distances(rows: readonly Row[], languages: readonly string[]): Distance[] {
  return languages
    .map((language) => {
      const pairs = rows.filter((r) => r.ours.length > 0 && (r.others.get(language)?.length ?? 0) > 0).slice(0, BASELINE_CONCEPTS);
      const theirs = (r: Row) => (r.others.get(language) ?? []).map((c) => c.form);
      const same = pairs.map((r) => closest(r.ours, theirs(r)));
      const ldn = same.length === 0 ? 1 : same.reduce((s, x) => s + x, 0) / same.length;
      let total = 0;
      let n = 0;
      for (let i = 0; i < pairs.length; i++)
        for (let j = 0; j < pairs.length; j++)
          if (i !== j) {
            total += closest(pairs[i]!.ours, theirs(pairs[j]!));
            n += 1;
          }
      const baseline = n === 0 ? 0 : total / n;
      return { language, compared: pairs.length, ldn, ldnd: baseline > 0 ? ldn / baseline : null };
    })
    .sort((a, b) => (a.ldnd ?? 9) - (b.ldnd ?? 9) || a.ldn - b.ldn || (a.language < b.language ? -1 : 1));
}

/** One language's line under the table. */
export function distanceLine(d: Distance): string {
  if (d.compared < MIN_COMPARED) return `${d.language}: ${d.compared} ${d.compared === 1 ? "concept" : "concepts"} with forms on both sides, too few to measure.`;
  const pct = (x: number) => `${Math.round(100 * x)}%`;
  const ldnd = d.ldnd === null ? "" : ` LDND ${pct(d.ldnd)}${d.ldnd >= 0.95 ? " (no closer than chance)" : ""}.`;
  return `${d.language}: over ${d.compared} concepts, words for the same thing differ in ${pct(d.ldn)} of their sounds (LDN).${ldnd}`;
}
