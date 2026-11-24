// Signs: the first questions asked of a script, known or not (the Glyphs
// kit's sign analysis). How many signs there are says what kind of script
// it may be (an alphabet has two to four dozen, a syllabary five to nine
// dozen, a logo-syllabic script hundreds); where each sign stands in a word
// sorts vowels and endings from the rest; which signs follow which, and the
// conditional entropy of the next sign (Rao et al. 2009), set writing that
// records a language apart from a rigid or a random sequence. Pure,
// npm-tested.

export type Word = { word: string; count: number };

/** How a word is cut into signs: each letter with its marks, or the pieces
 * a transliteration separates with hyphens or dots (`ko-no-so`, `a.ta`). */
export type SignMode = "letters" | "pieces";

const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });

/** A word's signs. Punctuation and spaces are no signs. */
export function signsOf(word: string, mode: SignMode): string[] {
  const w = word.normalize("NFC");
  if (mode === "pieces") return w.split(/[-.=\s]+/u).filter((s) => s !== "" && /[\p{L}\p{N}\p{So}]/u.test(s));
  return [...graphemes.segment(w)].map((g) => g.segment).filter((g) => /[\p{L}\p{N}\p{So}\p{M}]/u.test(g));
}

export type SignRow = { sign: string; count: number; words: number; initial: number; medial: number; final: number; alone: number; example: string };

/** Every sign with its running count, the word types it is in, where in the word it stands, and the commonest word with it. */
export function signTable(words: readonly Word[], mode: SignMode): SignRow[] {
  const rows = new Map<string, SignRow & { best: number }>();
  for (const w of words) {
    const s = signsOf(w.word, mode);
    const seen = new Set<string>();
    s.forEach((sign, i) => {
      const r = rows.get(sign) ?? { sign, count: 0, words: 0, initial: 0, medial: 0, final: 0, alone: 0, example: w.word, best: -1 };
      r.count += w.count;
      if (!seen.has(sign)) {
        r.words += 1;
        seen.add(sign);
      }
      if (s.length === 1) r.alone += w.count;
      else if (i === 0) r.initial += w.count;
      else if (i === s.length - 1) r.final += w.count;
      else r.medial += w.count;
      if (w.count > r.best) {
        r.best = w.count;
        r.example = w.word;
      }
      rows.set(sign, r);
    });
  }
  return [...rows.values()].map(({ best: _, ...r }) => r).sort((a, b) => b.count - a.count || (a.sign < b.sign ? -1 : 1));
}

/** Two signs in a row inside a word, with how often. */
export type Pair = { first: string; second: string; count: number };

export function signPairs(words: readonly Word[], mode: SignMode, limit = 40): Pair[] {
  const counts = new Map<string, Pair>();
  for (const w of words) {
    const s = signsOf(w.word, mode);
    for (let i = 0; i + 1 < s.length; i++) {
      const k = `${s[i]}\u0001${s[i + 1]}`;
      const p = counts.get(k) ?? { first: s[i]!, second: s[i + 1]!, count: 0 };
      p.count += w.count;
      counts.set(k, p);
    }
  }
  return [...counts.values()].sort((a, b) => b.count - a.count || (a.first + a.second < b.first + b.second ? -1 : 1)).slice(0, limit);
}

export type Entropy = {
  /** Distinct signs and running signs. */
  signs: number;
  tokens: number;
  /** Signs seen once: how much of the inventory may still be unseen. */
  once: number;
  /** Bits per sign, of the signs alone. */
  h1: number;
  /** Bits for the next sign given the one before (a word's edges as a sign `#`). */
  conditional: number;
};

const H = (counts: Iterable<number>): number => {
  const v = [...counts];
  const total = v.reduce((s, x) => s + x, 0);
  return total === 0 ? 0 : -v.reduce((s, x) => (x === 0 ? s : s + (x / total) * Math.log2(x / total)), 0);
};

/** The script's entropies over running signs. */
export function entropy(words: readonly Word[], mode: SignMode): Entropy {
  const single = new Map<string, number>();
  const pair = new Map<string, number>();
  const first = new Map<string, number>();
  for (const w of words) {
    const s = signsOf(w.word, mode);
    if (s.length === 0) continue;
    for (const x of s) single.set(x, (single.get(x) ?? 0) + w.count);
    const seq = ["#", ...s, "#"];
    for (let i = 0; i + 1 < seq.length; i++) {
      pair.set(`${seq[i]}\u0001${seq[i + 1]}`, (pair.get(`${seq[i]}\u0001${seq[i + 1]}`) ?? 0) + w.count);
      first.set(seq[i]!, (first.get(seq[i]!) ?? 0) + w.count);
    }
  }
  const tokens = [...single.values()].reduce((s, x) => s + x, 0);
  return { signs: single.size, tokens, once: [...single.values()].filter((x) => x === 1).length, h1: H(single.values()), conditional: H(pair.values()) - H(first.values()) };
}

/** What the number of signs suggests, with the usual caution. */
export function scriptNote(e: Entropy): string {
  if (e.tokens === 0) return "";
  const kind = e.signs <= 45 ? "an alphabet or an abjad (most have 20 to 40 signs)" : e.signs <= 100 ? "a syllabary (most have 40 to 90 signs; Linear B has about 90)" : "a logo-syllabic script, signs for words beside signs for sounds (they have hundreds)";
  const unseen = e.once > 0 ? ` ${e.once} ${e.once === 1 ? "sign is" : "signs are"} seen only once, so more may be unseen.` : "";
  const per = e.h1 === 0 ? "" : ` Each sign carries ${e.h1.toFixed(2)} bits; knowing the sign before leaves ${e.conditional.toFixed(2)}, ${Math.round((1 - e.conditional / e.h1) * 100)}% less. Writing that records a language sits between a fixed sequence (near 0% left) and signs in any order (no less).`;
  return `${e.signs} different signs in ${e.tokens.toLocaleString("en")} running signs: as many as ${kind}.${unseen}${per}`;
}

/** The share of a sign's uses at one place in the word, as a whole percent. */
export const placeShare = (n: number, r: SignRow): string => (r.count === 0 || n === 0 ? "–" : `${Math.round((100 * n) / r.count)}%`);

export function signsTsv(rows: readonly SignRow[]): string {
  return ["sign\tcount\tword_types\tinitial\tmedial\tfinal\talone\texample", ...rows.map((r) => [r.sign, r.count, r.words, r.initial, r.medial, r.final, r.alone, r.example].join("\t"))].join("\n") + "\n";
}
