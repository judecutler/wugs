// Entries that may be one word: two lexicon entries with the same meaning
// whose headwords are the same, differ only in accents, or are one letter
// apart (`kesi` and `kési`, both 'river'), the duplicates a lexicon gathers
// from imports, rapid elicitation and several people typing. Merging them is
// the person's call. Pure, npm-tested.

import { bare, oneApart, units } from "./spelling.ts";

export type EntryLike = { id: string; lemma: string; homograph: number | null; glosses: string[]; used: boolean };

export type LikelyPair = {
  /** The entry to keep: the one used in a text, else the one listed first. */
  keep: EntryLike;
  other: EntryLike;
  /** "same": the same headword; "accents": only marks differ; "letter": one letter added, left out or changed. */
  why: "same" | "accents" | "letter";
  /** The meanings they share, as `keep` writes them. */
  shared: string[];
};

/** The parts of a gloss compared: split at `;` `,` `/`, lower case, spaces collapsed, a leading "to " dropped. */
export function glossKeys(gloss: string): string[] {
  return gloss
    .split(/[;,/]/u)
    .map((g) =>
      g
        .normalize("NFC")
        .toLowerCase()
        .replace(/\s+/gu, " ")
        .trim()
        .replace(/^to /u, ""),
    )
    .filter((g) => g !== "");
}

/** Headwords shorter than this (in letters) are only compared when the same or differing in accents. */
const SHORTEST = 4;

function near(a: string, b: string): LikelyPair["why"] | null {
  const [x, y] = [a.normalize("NFC").toLowerCase(), b.normalize("NFC").toLowerCase()];
  if (x === y) return "same";
  if (bare(x) === bare(y)) return "accents";
  const [ux, uy] = [units(x), units(y)];
  return Math.min(ux.length, uy.length) >= SHORTEST && oneApart(ux, uy) ? "letter" : null;
}

const WHY_ORDER = { same: 0, accents: 1, letter: 2 } as const;

/** Every pair of entries that share a meaning and whose headwords are near,
 * the closest first; two numbered homographs of one headword are taken as
 * told apart on purpose. */
export function likelyDuplicates(entries: readonly EntryLike[], limit = 200): LikelyPair[] {
  const byGloss = new Map<string, number[]>();
  entries.forEach((e, i) => {
    for (const k of new Set(e.glosses.flatMap(glossKeys))) {
      const list = byGloss.get(k) ?? [];
      list.push(i);
      byGloss.set(k, list);
    }
  });
  const seen = new Set<string>();
  const out: LikelyPair[] = [];
  for (const list of byGloss.values()) {
    if (list.length < 2 || list.length > 50) continue;
    for (let p = 0; p < list.length; p++)
      for (let q = p + 1; q < list.length; q++) {
        const [i, j] = [list[p]!, list[q]!];
        const id = `${i}:${j}`;
        if (seen.has(id)) continue;
        seen.add(id);
        const [a, b] = [entries[i]!, entries[j]!];
        const why = near(a.lemma, b.lemma);
        if (why === null) continue;
        if (why === "same" && a.homograph !== null && b.homograph !== null) continue;
        const [keep, other] = b.used && !a.used ? [b, a] : [a, b];
        const theirs = new Set(other.glosses.flatMap(glossKeys));
        const shared = keep.glosses.filter((g) => glossKeys(g).some((k) => theirs.has(k)));
        out.push({ keep, other, why, shared });
      }
  }
  out.sort((x, y) => WHY_ORDER[x.why] - WHY_ORDER[y.why] || x.keep.lemma.localeCompare(y.keep.lemma));
  return out.slice(0, limit);
}

/** The line over the list. */
export function duplicatesLine(pairs: readonly LikelyPair[], entries: number): string {
  if (entries === 0) return "The lexicon has no entries yet.";
  if (pairs.length === 0) return `No two of the ${entries} entries share a meaning with the same or a nearly same headword.`;
  const n = pairs.length === 1 ? "1 pair of entries shares" : `${pairs.length} pairs of entries share`;
  return `${n} a meaning and a headword that is the same or one letter apart. They may be one word entered twice, or two words: merge only the ones that are the same.`;
}

/** Why a pair is listed, in words. */
export function duplicateNote(p: LikelyPair): string {
  const how = p.why === "same" ? "the same headword" : p.why === "accents" ? "headwords that differ only in accents" : "headwords one letter apart";
  return `${how}, both ‘${p.shared.join("’, ‘")}’${p.keep.used && !p.other.used ? "; only the first is used in a text" : ""}`;
}
