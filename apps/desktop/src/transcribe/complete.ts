// Word completion while transcribing: the commonest word of the project
// that starts with what is being typed, offered beside the field and taken
// with → at the end of the line, as a shell's suggestions are. Pure: no DOM.

/** The project's words, sorted for prefix search, with their counts (lower case, NFC). */
export type CompletionIndex = { words: string[]; counts: Map<string, number> };

/** The shortest start a word is completed from, in characters. */
export const SHORTEST_START = 2;
/** A word must have been used this often to be offered. */
export const MIN_USES = 2;

export function completionIndex(counts: Map<string, number>): CompletionIndex {
  return { words: [...counts.keys()].sort(), counts };
}

/** Add a word to the index (a saved transcription's new word). */
export function addWord(index: CompletionIndex, word: string): void {
  const w = word.normalize("NFC").toLowerCase();
  if (!/\p{L}/u.test(w)) return;
  if (!index.counts.has(w)) {
    let lo = 0;
    let hi = index.words.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (index.words[mid]! < w) lo = mid + 1;
      else hi = mid;
    }
    index.words.splice(lo, 0, w);
  }
}

/** The word being typed: letters, marks and word-inner apostrophes and hyphens right before the caret. */
export function wordAt(text: string, caret: number): { start: number; word: string } {
  const before = text.slice(0, caret);
  const m = /[\p{L}\p{M}\p{N}'ʼ’-]+$/u.exec(before);
  return m === null ? { start: caret, word: "" } : { start: m.index, word: m[0] };
}

/** The commonest known word that `typed` starts (capitals and accents as written; case aside), longer than it, or null. */
export function complete(index: CompletionIndex, typed: string): { word: string; count: number } | null {
  const t = typed.normalize("NFC").toLowerCase();
  if ([...t].length < SHORTEST_START) return null;
  let lo = 0;
  let hi = index.words.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (index.words[mid]! < t) lo = mid + 1;
    else hi = mid;
  }
  let best: { word: string; count: number } | null = null;
  for (let i = lo; i < index.words.length && index.words[i]!.startsWith(t); i++) {
    const w = index.words[i]!;
    const n = index.counts.get(w) ?? 0;
    if (w === t || n < MIN_USES) continue;
    if (best === null || n > best.count) best = { word: w, count: n };
  }
  return best;
}

/** The field's text with the word at the caret completed, and where the caret goes. */
export function accept(text: string, caret: number, word: string): { text: string; caret: number } {
  const { start, word: typed } = wordAt(text, caret);
  // The typed start keeps its capitals: «Ka» + kanima → «Kanima».
  const shown = typed + word.slice(typed.length);
  const next = text.slice(0, start) + shown + text.slice(caret);
  return { text: next, caret: start + shown.length };
}
