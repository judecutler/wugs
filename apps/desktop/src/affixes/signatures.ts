// Endings (or beginnings) guessed from the words alone, before anything is
// glossed: Goldsmith's signatures (Linguistica, 2001) in a simple form. Every
// word is cut into a stem and an ending of up to MAX_AFFIX letters; a stem
// met with two or more endings keeps the endings that MIN_STEMS stems share;
// stems with the same set of endings make one signature, as `walk` and `talk`
// share ∅.s.ed.ing. A guide to where to look, never written anywhere.
import { units } from "../checks/spelling.ts";

export type WordCount = { word: string; count: number };
export type Side = "suffix" | "prefix";

export type Signature = {
  /** The endings in the order `compareAffix` gives, "" for none (∅). */
  affixes: string[];
  /** Its stems, most used first. */
  stems: { stem: string; count: number }[];
  /** Running words of every stem with every ending. */
  tokens: number;
};

export type Guessed = {
  signatures: Signature[];
  /** Each kept ending with how many stems take it, most first. */
  affixes: { affix: string; stems: number }[];
  /** Word types read (letters only, at most WORDS_READ). */
  read: number;
};

/** The longest ending tried, in letters. */
export const MAX_AFFIX = 4;
/** The shortest stem, in letters. */
export const MIN_STEM = 3;
/** Stems an ending must be met on, and a signature must have. */
export const MIN_STEMS = 3;
/** The commonest word types read. */
export const WORDS_READ = 50_000;
/** Signatures returned. */
export const MAX_SIGNATURES = 60;

const LETTERS = /^[\p{L}\p{M}]+$/u;

/** Shorter endings first, then by alphabet; ∅ first of all. */
const compareAffix = (a: string, b: string) => units(a).length - units(b).length || (a < b ? -1 : a > b ? 1 : 0);

/** Signatures over word types with their counts, for endings (suffix) or beginnings (prefix). */
export function signatures(words: readonly WordCount[], side: Side): Guessed {
  const counts = new Map<string, number>();
  for (const w of [...words].sort((a, b) => b.count - a.count)) {
    if (counts.size >= WORDS_READ) break;
    const key = w.word.normalize("NFC").toLowerCase();
    if (!LETTERS.test(key)) continue;
    counts.set(key, (counts.get(key) ?? 0) + w.count);
  }
  // Work on letter lists, reversed for prefixes so both sides cut at the end.
  const letters = (w: string) => (side === "suffix" ? units(w) : units(w).reverse());
  const join = (u: string[]) => (side === "suffix" ? u : [...u].reverse()).join("");
  const endings = new Map<string, Set<string>>(); // stem → endings
  const add = (stem: string, ending: string) => {
    let s = endings.get(stem);
    if (s === undefined) endings.set(stem, (s = new Set()));
    s.add(ending);
  };
  for (const word of counts.keys()) {
    const u = letters(word);
    for (let k = 1; k <= MAX_AFFIX && u.length - k >= MIN_STEM; k++) add(join(u.slice(0, u.length - k)), join(u.slice(u.length - k)));
  }
  // A stem that is a word on its own takes ∅.
  for (const [stem, set] of endings) if (counts.has(stem)) set.add("");
  const stemsOf = new Map<string, number>();
  for (const set of endings.values()) if (set.size >= 2) for (const e of set) stemsOf.set(e, (stemsOf.get(e) ?? 0) + 1);
  const kept = new Set([...stemsOf].filter(([, n]) => n >= MIN_STEMS).map(([e]) => e));
  const bySignature = new Map<string, Signature>();
  const word = (stem: string, e: string) => (side === "suffix" ? stem + e : e + stem);
  for (const [stem, set] of endings) {
    const mine = [...set].filter((e) => kept.has(e)).sort(compareAffix);
    if (mine.length < 2) continue;
    const key = mine.join("\u001f");
    let sig = bySignature.get(key);
    if (sig === undefined) bySignature.set(key, (sig = { affixes: mine, stems: [], tokens: 0 }));
    const n = mine.reduce((sum, e) => sum + (counts.get(word(stem, e)) ?? 0), 0);
    sig.stems.push({ stem, count: n });
    sig.tokens += n;
  }
  const sigs = [...bySignature.values()].filter((s) => s.stems.length >= MIN_STEMS);
  for (const s of sigs) s.stems.sort((a, b) => b.count - a.count || (a.stem < b.stem ? -1 : 1));
  // Most stems times endings first; on a tie the shorter endings (the longer
  // stems), so kanö-la beats kan-öla, then the most running words.
  const affixLetters = (s: Signature) => s.affixes.reduce((n, e) => n + units(e).length, 0);
  sigs.sort((a, b) => b.stems.length * (b.affixes.length - 1) - a.stems.length * (a.affixes.length - 1) || affixLetters(a) - affixLetters(b) || b.tokens - a.tokens);
  // Each word is read one way: a stem keeps its place only in the best
  // signature none of its words were claimed by, so walk.s.ed.ing does not
  // come back as wal.ks.ked.king.
  const claimed = new Set<string>();
  const top: Signature[] = [];
  for (const sig of sigs) {
    if (top.length >= MAX_SIGNATURES) break;
    const free = sig.stems.filter((x) => sig.affixes.every((e) => !claimed.has(word(x.stem, e))));
    if (free.length < MIN_STEMS) continue;
    for (const x of free) for (const e of sig.affixes) claimed.add(word(x.stem, e));
    top.push({ affixes: sig.affixes, stems: free, tokens: free.reduce((n, x) => n + x.count, 0) });
  }
  const used = new Map<string, number>();
  for (const s of top) for (const e of s.affixes) if (e !== "") used.set(e, (used.get(e) ?? 0) + s.stems.length);
  const affixes = [...used].map(([affix, stems]) => ({ affix, stems })).sort((a, b) => b.stems - a.stems || compareAffix(a.affix, b.affix));
  return { signatures: top, affixes, read: counts.size };
}

/** An ending as a grammar writes it: `-ed`, `un-`, ∅ for none. */
export const affixLabel = (affix: string, side: Side): string => (affix === "" ? "∅" : side === "suffix" ? `-${affix}` : `${affix}-`);

/** A signature as Linguistica writes it: `∅.ed.ing.s`. */
export const signatureLabel = (s: Signature): string => s.affixes.map((a) => (a === "" ? "∅" : a)).join(".");

/** The summary above the list. */
export function guessedLine(g: Guessed, side: Side): string {
  const what = side === "suffix" ? "endings" : "beginnings";
  if (g.read === 0) return "No words to read yet: transcribe some sentences first.";
  if (g.signatures.length === 0) return `No ${what} are shared by ${MIN_STEMS} or more stems in ${g.read} word ${g.read === 1 ? "form" : "forms"}.`;
  const top = g.affixes.slice(0, 6).map((a) => `${affixLabel(a.affix, side)} (${a.stems})`).join(", ");
  return `${g.signatures.length} ${g.signatures.length === 1 ? "set" : "sets"} of ${what} shared by ${MIN_STEMS} or more stems, from ${g.read} word ${g.read === 1 ? "form" : "forms"}. Met on most stems: ${top}.`;
}

/** Every signature as a table: the endings, how many stems, running words, the stems. */
export function signaturesTsv(g: Guessed, side: Side): string {
  const rows = g.signatures.map((s) => [s.affixes.map((a) => affixLabel(a, side)).join(" "), String(s.stems.length), String(s.tokens), s.stems.map((x) => x.stem).join(", ")].join("\t"));
  return [[side === "suffix" ? "Endings" : "Beginnings", "Stems", "Words", "Stem list"].join("\t"), ...rows].join("\n") + "\n";
}
