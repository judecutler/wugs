// Near spellings: rare words that are one letter, or only accents, away from
// a common word, the typing slips and spelling variants worth a look before a
// dictionary or a deposit (`kési` ×1 beside `kesi` ×40). Read from the word
// types the Sounds panel reads (src-tauri/src/sounds.rs); pure, npm-tested.

export type WordCount = { word: string; count: number };

export type NearPair = {
  rare: string;
  rareCount: number;
  common: string;
  commonCount: number;
  /** "accents": the same letters with other marks; "letter": one letter added, left out or changed. */
  why: "accents" | "letter";
};

const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });
const units = (w: string): string[] => Array.from(graphemes.segment(w), (g) => g.segment);
const bare = (w: string): string => w.normalize("NFD").replace(/\p{M}/gu, "").normalize("NFC");

/** Whether two grapheme lists are one insertion, deletion or substitution apart. */
export function oneApart(a: readonly string[], b: readonly string[]): boolean {
  if (Math.abs(a.length - b.length) > 1) return false;
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  if (a.length === b.length) return i < a.length && a.slice(i + 1).join("\u0000") === b.slice(i + 1).join("\u0000");
  const [long, short] = a.length > b.length ? [a, b] : [b, a];
  return long.slice(i + 1).join("\u0000") === short.slice(i).join("\u0000");
}

/** A word is rare beside another when it is used at most `rareAt` times and
 * the other at least `ratio` times as often (and at least 4 times). */
const RARE_AT = 2;
const RATIO = 4;
/** Words shorter than this (in letters) have too many neighbours to judge. */
const SHORTEST = 4;
/** At most this many word types are compared (the commonest). */
export const TYPES_READ = 20_000;

/** Pairs of a rare word and a common one it may be a slip for, the commonest first. */
export function nearSpellings(words: readonly WordCount[], limit = 200): NearPair[] {
  // Case is not a spelling difference here: capitals at a sentence start are expected.
  const counts = new Map<string, number>();
  for (const { word, count } of words) {
    const w = word.normalize("NFC").toLowerCase();
    if (!/\p{L}/u.test(w)) continue;
    counts.set(w, (counts.get(w) ?? 0) + count);
  }
  const types = [...counts.entries()].sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1)).slice(0, TYPES_READ);
  const found = new Map<string, NearPair>();
  const consider = (x: string, y: string, why: NearPair["why"]) => {
    const [cx, cy] = [counts.get(x)!, counts.get(y)!];
    const [rare, common] = cx <= cy ? [x, y] : [y, x];
    const [rc, cc] = [counts.get(rare)!, counts.get(common)!];
    if (rare === common || rc > RARE_AT || cc < Math.max(4, RATIO * rc)) return;
    const prev = found.get(rare);
    // A rare word keeps its likeliest partner: accents before letters, then the commonest.
    if (prev !== undefined && (prev.why === "accents" ? 1 : 0) > (why === "accents" ? 1 : 0)) return;
    if (prev !== undefined && prev.why === why && prev.commonCount >= cc) return;
    found.set(rare, { rare, rareCount: rc, common, commonCount: cc, why });
  };
  // Only accents differ: same bare form.
  const byBare = new Map<string, string[]>();
  for (const [w] of types) byBare.set(bare(w), [...(byBare.get(bare(w)) ?? []), w]);
  for (const group of byBare.values()) for (let i = 0; i < group.length; i++) for (let j = i + 1; j < group.length; j++) consider(group[i]!, group[j]!, "accents");
  // One letter apart: words sharing a form with one letter left out (symmetric deletion).
  const split = new Map(types.map(([w]) => [w, units(w)]));
  const byKey = new Map<string, string[]>();
  for (const [w, us] of split) {
    if (us.length < SHORTEST - 1) continue;
    const keys = new Set<string>();
    for (let i = 0; i < us.length; i++) keys.add(`${i}\u0001${[...us.slice(0, i), ...us.slice(i + 1)].join("")}`);
    // The whole word stands for "nothing left out", matched against others' deletions at any place.
    for (let i = 0; i <= us.length; i++) keys.add(`${i}\u0001${us.join("")}`);
    for (const k of keys) byKey.set(k, [...(byKey.get(k) ?? []), w]);
  }
  for (const group of byKey.values()) {
    if (group.length < 2 || group.length > 50) continue;
    for (let i = 0; i < group.length; i++)
      for (let j = i + 1; j < group.length; j++) {
        const [a, b] = [group[i]!, group[j]!];
        const [ua, ub] = [split.get(a)!, split.get(b)!];
        if (Math.max(ua.length, ub.length) < SHORTEST || bare(a) === bare(b)) continue;
        if (oneApart(ua, ub)) consider(a, b, "letter");
      }
  }
  return [...found.values()].sort((x, y) => y.commonCount - x.commonCount || x.rare.localeCompare(y.rare)).slice(0, limit);
}

/** What a pair's card says under the two words. */
export function pairNote(p: NearPair): string {
  return p.why === "accents" ? "The same letters with other accents or marks." : "One letter added, left out or changed.";
}

/** The line above the cards. */
export function spellingSummary(pairs: readonly NearPair[], read: number): string {
  if (read === 0) return "No transcribed words yet.";
  if (pairs.length === 0) return `No rare word in the ${read.toLocaleString("en")} different words looks like a slip for a common one.`;
  return `${pairs.length} rare word${pairs.length === 1 ? "" : "s"} ${pairs.length === 1 ? "is" : "are"} one letter, or only accents, away from a common word. Some are real words; open each to check, and fix slips with Replace… in the Search panel.`;
}
