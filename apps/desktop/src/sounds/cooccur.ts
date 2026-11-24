// Which vowels come together in a word, the first look at vowel harmony:
// each vowel against the next vowel in the same word (the consonants
// between skipped), counted over word types, as observed over expected
// (Pierrehumbert's O/E: 1 is chance, below 1 avoided, above 1 preferred).
// The same over consonants that follow each other across a vowel shows
// restrictions such as Semitic roots avoiding two consonants of one place.
// Pure, npm-tested.
import { sounds } from "./ipa.ts";
import { classOf } from "./shapes.ts";

export type Word = { word: string; count: number };

/** V: each vowel and the next vowel; C: each consonant and the next consonant; CV: a consonant and the vowel right after it. */
export type Together = "V" | "C" | "CV";

export type Cooccurrence = {
  /** The first sounds (rows), commonest first. */
  sounds: string[];
  /** The second sounds (columns): the same as `sounds` for V and C, the vowels for CV. */
  columns: string[];
  /** observed[i][j]: word types in which `sounds[i]` is followed by `columns[j]`. */
  observed: number[][];
  /** What the two sounds' shares alone predict, and O/E per cell (null when nothing is expected). */
  expected: number[][];
  ratio: (number | null)[][];
  /** Pairs counted. */
  total: number;
};

/** The sounds of class `kind` in a word, in order (vowels: the vowels, the consonants between skipped). */
const ofClass = (word: string, kind: "V" | "C"): string[] => sounds(word.normalize("NFC").toLocaleLowerCase()).filter((s) => classOf(s) === kind);

/** The pairs a word gives: next of one class (V, C), or each consonant with a vowel straight after it (CV). */
function pairsOf(word: string, kind: Together): [string, string][] {
  if (kind !== "CV") {
    const s = ofClass(word, kind);
    return s.slice(1).map((b, i) => [s[i]!, b]);
  }
  const s = sounds(word);
  const out: [string, string][] = [];
  for (let i = 0; i + 1 < s.length; i++) if (classOf(s[i]!) === "C" && classOf(s[i + 1]!) === "V") out.push([s[i]!, s[i + 1]!]);
  return out;
}

/** Each `kind` sound against the next one in the same word, over word types. */
export function cooccurrence(words: readonly Word[], kind: Together, max = 14): Cooccurrence {
  const pairs = new Map<string, number>();
  const firsts = new Map<string, number>();
  const seconds = new Map<string, number>();
  const seenType = new Set<string>();
  let total = 0;
  for (const w of words) {
    const key = w.word.normalize("NFC").toLocaleLowerCase();
    if (seenType.has(key)) continue;
    seenType.add(key);
    for (const [a, b] of pairsOf(key, kind)) {
      pairs.set(`${a}\u0001${b}`, (pairs.get(`${a}\u0001${b}`) ?? 0) + 1);
      firsts.set(a, (firsts.get(a) ?? 0) + 1);
      seconds.set(b, (seconds.get(b) ?? 0) + 1);
      total += 1;
    }
  }
  const commonest = (m: Map<string, number>) => [...m.entries()].sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1)).slice(0, max).map(([s]) => s);
  const all = new Map<string, number>();
  for (const [s, n] of firsts) all.set(s, (all.get(s) ?? 0) + n);
  for (const [s, n] of seconds) all.set(s, (all.get(s) ?? 0) + n);
  const list = kind === "CV" ? commonest(firsts) : commonest(all);
  const columns = kind === "CV" ? commonest(seconds) : list;
  const observed = list.map((a) => columns.map((b) => pairs.get(`${a}\u0001${b}`) ?? 0));
  const expected = list.map((a) => columns.map((b) => (total === 0 ? 0 : ((firsts.get(a) ?? 0) * (seconds.get(b) ?? 0)) / total)));
  const ratio = expected.map((row, i) => row.map((e, j) => (e === 0 ? null : observed[i]![j]! / e)));
  return { sounds: list, columns, observed, expected, ratio, total };
}

/** Cells worth a word: expected at least `minExpected` times, and well under or over chance. */
export function standouts(c: Cooccurrence, minExpected = 3): { a: string; b: string; ratio: number; observed: number }[] {
  const out: { a: string; b: string; ratio: number; observed: number }[] = [];
  c.sounds.forEach((a, i) =>
    c.columns.forEach((b, j) => {
      const r = c.ratio[i]![j];
      if (r === null || r === undefined) return;
      if (c.expected[i]![j]! >= minExpected && (r <= 0.5 || r >= 1.8)) out.push({ a, b, ratio: r, observed: c.observed[i]![j]! });
    }),
  );
  return out.sort((x, y) => Math.abs(Math.log(y.ratio + 0.05)) - Math.abs(Math.log(x.ratio + 0.05)));
}

/** The table's note: what stands out, in words. */
export function cooccurrenceNote(c: Cooccurrence, kind: Together): string {
  if (c.total === 0) return kind === "V" ? "No word has two vowels yet." : kind === "C" ? "No word has two consonants yet." : "No word has a consonant before a vowel yet.";
  const s = standouts(c).slice(0, 6);
  const join = kind === "CV" ? "" : "…";
  const words = s.map((x) => `${x.a}${join}${x.b} ${x.ratio === 0 ? "never met" : x.ratio < 1 ? "avoided" : "preferred"} (${x.ratio.toFixed(1)})`);
  const what = kind === "V" ? "a vowel and the next vowel" : kind === "C" ? "a consonant and the next consonant" : "a consonant and the vowel right after it";
  const head = `${c.total.toLocaleString("en")} pairs of ${what} in a word, counted over word types; 1 is chance.${kind === "CV" ? " A gap where many are expected (a palatal never before a back vowel, say) is a restriction to explain; one where few are expected may be chance." : ""}`;
  return words.length === 0 ? `${head} No pair stands well apart from chance yet.` : `${head} Standing out: ${words.join(", ")}.`;
}

export function cooccurrenceTsv(c: Cooccurrence): string {
  return [["", ...c.columns].join("\t"), ...c.sounds.map((a, i) => [a, ...c.ratio[i]!.map((r, j) => (r === null ? "" : `${r.toFixed(2)} (${c.observed[i]![j]})`))].join("\t"))].join("\n") + "\n";
}
