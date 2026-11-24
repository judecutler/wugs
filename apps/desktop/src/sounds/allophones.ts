// Possible allophones: pairs of similar sounds (one or two distinctive
// features apart) that never occur in the same environment in the project's
// words, each with the rule the Environments section would suggest. The
// pairs a phonologist checks first when working out the phonemes. Pure; no DOM.
import { environments, EDGE, suggestedRule } from "./environments.ts";
import { FEATURES, featuresOf } from "./features.ts";
import { sounds } from "./ipa.ts";

export type Allophones = { a: string; b: string; apart: number; countA: number; countB: number; rule: string | null };

/** Sounds this many times or fewer are too rare to judge. */
export const MIN_COUNT = 5;
/** At most this many features apart. */
export const MAX_APART = 2;

/** How many distinctive features two sounds differ in (null when either is off the chart). */
export function featureDistance(a: string, b: string): number | null {
  const fa = featuresOf(a);
  const fb = featuresOf(b);
  if (fa === null || fb === null) return null;
  return FEATURES.filter((f) => fa[f] !== fb[f]).length;
}

/** Every pair of similar, common sounds that never share an environment (the
 * sound before and after, a word edge counting as one), closest first. */
export function possibleAllophones(words: readonly { word: string; count: number }[], minCount = MIN_COUNT, maxApart = MAX_APART): Allophones[] {
  const envs = new Map<string, Set<string>>();
  const counts = new Map<string, number>();
  for (const { word, count } of words) {
    const s = sounds(word);
    s.forEach((x, i) => {
      counts.set(x, (counts.get(x) ?? 0) + count);
      const set = envs.get(x) ?? new Set<string>();
      set.add(`${s[i - 1] ?? EDGE}\u001f${s[i + 1] ?? EDGE}`);
      envs.set(x, set);
    });
  }
  const common = [...counts].filter(([, n]) => n >= minCount).map(([x]) => x).filter((x) => featuresOf(x) !== null);
  common.sort((x, y) => counts.get(y)! - counts.get(x)! || x.localeCompare(y));
  const out: Allophones[] = [];
  for (let i = 0; i < common.length; i++) {
    for (let j = i + 1; j < common.length; j++) {
      const [a, b] = [common[i]!, common[j]!];
      const apart = featureDistance(a, b);
      if (apart === null || apart === 0 || apart > maxApart) continue;
      const ea = envs.get(a)!;
      let shared = false;
      for (const e of envs.get(b)!) if (ea.has(e)) {
        shared = true;
        break;
      }
      if (shared) continue;
      const rule = suggestedRule(environments(words, a, b));
      out.push({ a, b, apart, countA: counts.get(a)!, countB: counts.get(b)!, rule: rule?.rule ?? null });
    }
  }
  return out.sort((x, y) => x.apart - y.apart || Math.min(y.countA, y.countB) - Math.min(x.countA, x.countB) || x.a.localeCompare(y.a) || x.b.localeCompare(y.b));
}

/** The section's line. */
export function allophonesLine(list: readonly Allophones[]): string {
  if (list.length === 0) return `No two similar sounds (used ${MIN_COUNT} times or more, ${MAX_APART} features apart or fewer) keep to separate environments in these words.`;
  return `${list.length} ${list.length === 1 ? "pair" : "pairs"} of similar sounds never share an environment in these words: candidates for allophones of one phoneme. More words may bring them together, so check each.`;
}
