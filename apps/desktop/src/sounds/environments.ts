// Environments of two sounds: what comes before and after each, counted in
// running words, so a linguist can see whether the two ever share an
// environment (and may contrast) or never do (and may be allophones in
// complementary distribution), and which side decides it. Read from the
// word types the Sounds panel reads; pure, npm-tested.

import { sounds } from "./ipa.ts";

/** A word's edge, as a neighbour. */
export const EDGE = "#";

export type EnvRow = { left: string; right: string; a: number; b: number; exampleA: string; exampleB: string };
export type NeighbourRow = { neighbour: string; a: number; b: number };
export type Environments = { a: string; b: string; countA: number; countB: number; whole: EnvRow[]; before: NeighbourRow[]; after: NeighbourRow[] };

/** Every environment of `a` and of `b` in the words, by running count:
 * whole environments (the sound before and after), and each side alone
 * (`before`: the sound that follows, as in "before i"; `after`: the one
 * that comes first). */
export function environments(words: readonly { word: string; count: number }[], a: string, b: string): Environments {
  const whole = new Map<string, EnvRow>();
  const before = new Map<string, NeighbourRow>();
  const after = new Map<string, NeighbourRow>();
  let countA = 0;
  let countB = 0;
  const bump = (m: Map<string, NeighbourRow>, n: string, isA: boolean, count: number) => {
    const row = m.get(n) ?? { neighbour: n, a: 0, b: 0 };
    if (isA) row.a += count;
    else row.b += count;
    m.set(n, row);
  };
  for (const { word, count } of words) {
    const s = sounds(word);
    s.forEach((x, i) => {
      if (x !== a && x !== b) return;
      const isA = x === a;
      if (isA) countA += count;
      else countB += count;
      const left = s[i - 1] ?? EDGE;
      const right = s[i + 1] ?? EDGE;
      const key = `${left}\u001f${right}`;
      const row = whole.get(key) ?? { left, right, a: 0, b: 0, exampleA: "", exampleB: "" };
      if (isA) {
        row.a += count;
        if (row.exampleA === "") row.exampleA = word;
      } else {
        row.b += count;
        if (row.exampleB === "") row.exampleB = word;
      }
      whole.set(key, row);
      bump(before, right, isA, count);
      bump(after, left, isA, count);
    });
  }
  const byUse = <T extends { a: number; b: number }>(rows: Iterable<T>): T[] => [...rows].sort((x, y) => y.a + y.b - (x.a + x.b));
  return { a, b, countA, countB, whole: byUse(whole.values()), before: byUse(before.values()), after: byUse(after.values()) };
}

/** An environment written as phonologists do: `a_i`, `#_a`. */
export const envLabel = (r: { left: string; right: string }): string => `${r.left}_${r.right}`;

const list = (xs: readonly string[]): string => (xs.length <= 6 ? xs.join(", ") : `${xs.slice(0, 6).join(", ")} and ${xs.length - 6} more`);

/** What the counts suggest, in words: whether the two share an environment,
 * and when they never do, whether one side alone tells them apart. */
export function distributionNote(e: Environments): string {
  if (e.a === e.b) return "Choose two different sounds.";
  const missing = [e.countA === 0 ? e.a : null, e.countB === 0 ? e.b : null].filter((x) => x !== null);
  if (missing.length > 0) return `«${missing.join("» and «")}» ${missing.length === 1 ? "is" : "are"} not in these words.`;
  const shared = e.whole.filter((r) => r.a > 0 && r.b > 0);
  if (shared.length > 0)
    return `«${e.a}» and «${e.b}» share ${shared.length} ${shared.length === 1 ? "environment" : "environments"} (${list(shared.map(envLabel))}): there they contrast, or vary freely. Minimal pairs tell which.`;
  const sides: string[] = [];
  for (const [rows, word] of [
    [e.before, "before"],
    [e.after, "after"],
  ] as const) {
    if (rows.some((r) => r.a > 0 && r.b > 0)) continue;
    const of = (pick: (r: NeighbourRow) => number) => list(rows.filter((r) => pick(r) > 0).map((r) => r.neighbour));
    sides.push(`«${e.a}» only ${word} ${of((r) => r.a)}, «${e.b}» only ${word} ${of((r) => r.b)}`);
  }
  const decided = sides.length === 0 ? " Neither side alone tells them apart: look at both together." : ` ${sides.join("; ")}.`;
  return `«${e.a}» and «${e.b}» never occur in the same environment in these words, so they may be allophones of one sound in complementary distribution.${decided}`;
}

/** The whole environments as a table for a spreadsheet. */
export function environmentsTsv(e: Environments): string {
  const rows = e.whole.map((r) => [envLabel(r), r.a, r.b, r.exampleA, r.exampleB].join("\t"));
  return [["Environment", e.a, e.b, `Example of ${e.a}`, `Example of ${e.b}`].join("\t"), ...rows].join("\n") + "\n";
}
