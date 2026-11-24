// Greenberg's (1960) quantitative indices of morphological type, from the
// glossed words (src-tauri/src/affixes.rs `Indices`): synthesis (morphs per
// word), compounding (roots per word), prefixing and suffixing (per word)
// and isolation (share of one-morph words). Pure; no DOM.

export type Indices = { words: number; morphs: number; roots: number; prefixes: number; suffixes: number; infixes: number; clitics: number; single: number };

/** Fewer glossed words than this, and the indices say little. */
export const MIN_WORDS = 100;

/** Greenberg's own figures for comparison (synthesis, M/W). */
export const REFERENCE: [string, number][] = [
  ["Vietnamese", 1.06],
  ["English", 1.68],
  ["Swahili", 2.55],
  ["Sanskrit", 2.59],
  ["West Greenlandic", 3.72],
];

export type IndexRow = { name: string; ratio: string; value: number; about: string };

export function indexRows(ix: Indices): IndexRow[] {
  if (ix.words === 0) return [];
  const per = (n: number) => n / ix.words;
  const rows: IndexRow[] = [
    { name: "Synthesis", ratio: "M/W", value: per(ix.morphs), about: "morphs per word" },
    { name: "Compounding", ratio: "R/W", value: per(ix.roots), about: "roots per word" },
    { name: "Prefixing", ratio: "P/W", value: per(ix.prefixes), about: "prefixes per word" },
    { name: "Suffixing", ratio: "S/W", value: per(ix.suffixes), about: "suffixes per word" },
  ];
  if (ix.infixes > 0) rows.push({ name: "Infixing", ratio: "In/W", value: per(ix.infixes), about: "infixes per word" });
  if (ix.clitics > 0) rows.push({ name: "Clitics", ratio: "Cl/W", value: per(ix.clitics), about: "clitics per word (counted as morphs in synthesis)" });
  rows.push({ name: "Isolation", ratio: "1/W", value: per(ix.single), about: "share of words of one morph" });
  return rows;
}

/** Greenberg's bands for synthesis. */
export function synthesisType(m: number): string {
  return m < 2 ? "analytic" : m < 3 ? "synthetic" : "polysynthetic";
}

/** What the indices say, in words. */
export function indicesNote(ix: Indices): string {
  if (ix.words === 0) return "No word is broken into morphs yet.";
  const m = ix.morphs / ix.words;
  const near = [...REFERENCE].sort((a, b) => Math.abs(a[1] - m) - Math.abs(b[1] - m))[0]!;
  const parts = [`Over ${ix.words.toLocaleString("en")} glossed ${ix.words === 1 ? "word" : "words"}, ${m.toFixed(2)} morphs a word: ${synthesisType(m)} in Greenberg's (1960) terms (analytic below 2, polysynthetic from 3), nearest of his samples to ${near[0]} (${near[1].toFixed(2)}).`];
  const affixes = ix.prefixes + ix.suffixes;
  if (affixes > 0) {
    const s = (100 * ix.suffixes) / affixes;
    parts.push(s >= 80 ? `Strongly suffixing: ${Math.round(s)}% of prefixes and suffixes are suffixes.` : s <= 20 ? `Strongly prefixing: ${Math.round(100 - s)}% of prefixes and suffixes are prefixes.` : `Both prefixing and suffixing: ${Math.round(s)}% of prefixes and suffixes are suffixes.`);
  }
  if (ix.words < MIN_WORDS) parts.push(`With fewer than ${MIN_WORDS} glossed words, read these as a first look.`);
  parts.push("The figures follow how the words were broken up: clitics count as morphs, and unglossed words are left out.");
  return parts.join(" ");
}

export function indicesTsv(ix: Indices): string {
  return `index\tratio\tvalue\tmeaning\n${indexRows(ix).map((r) => [r.name, r.ratio, r.value.toFixed(3), r.about].join("\t")).join("\n")}\n`;
}
