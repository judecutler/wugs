// Helpers for the Transcribe panel's "did you mean" offer. Pure, npm-tested.

/** `word` with the typed word's capital: a sentence-initial `Kési` offers `Kesi`. */
export function matchCase(typed: string, word: string): string {
  const first = typed.charAt(0);
  if (first !== first.toLowerCase() && first === first.toUpperCase()) return word.charAt(0).toUpperCase() + word.slice(1);
  return word;
}

const LETTER = /[\p{L}\p{M}\p{N}]/u;

/** The text with the first whole-word occurrence of `from` (NFC) written as `to`. */
export function replaceWord(text: string, from: string, to: string): string {
  const t = text.normalize("NFC");
  const f = from.normalize("NFC");
  let at = t.indexOf(f);
  while (at !== -1) {
    const before = at === 0 ? "" : t[at - 1]!;
    const after = t.slice(at + f.length, at + f.length + 1);
    if (!LETTER.test(before) && !LETTER.test(after)) return t.slice(0, at) + to + t.slice(at + f.length);
    at = t.indexOf(f, at + 1);
  }
  return t;
}
