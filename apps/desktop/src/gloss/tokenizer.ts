// Break a transcription into words (GLOSSING-MODEL, Gloss panel). Pure: no DOM.
// Whitespace separates words. Punctuation at a word's edge is stripped and kept
// beside it. Inside a word, everything stays: `-` `=` `~` `<` `>` are morph
// markers, and the apostrophe, glottal stop, ejective and other modifier
// letters are part of the spelling.

export type Word = {
  /** NFC. What a Token's `form` holds. */
  form: string;
  /** Punctuation stripped from the front and the back, in order. */
  lead: string;
  trail: string;
};

const MARKERS = new Set(["-", "=", "~", "<", ">"]);
// IPA tone letters and other modifier symbols are Sk, not letters, but they spell words.
const MODIFIER_SYMBOL = /^[\u02b0-\u02ff\ua700-\ua71f]$/u;
const WORD_CHAR = /^[\p{L}\p{M}\p{N}]$/u;
const APOSTROPHES = new Set(["'", "\u2019"]);
const QUOTE_CLOSE = APOSTROPHES;

const isWordChar = (c: string): boolean => WORD_CHAR.test(c) || MODIFIER_SYMBOL.test(c) || MARKERS.has(c);

/** A chunk with no letter, mark or number in it is punctuation alone ("—", "...", "-"). */
const hasSubstance = (chunk: string): boolean => [...chunk].some((c) => WORD_CHAR.test(c) || MODIFIER_SYMBOL.test(c));

function split(chunk: string): { lead: string; core: string; trail: string } {
  const chars = [...chunk];
  let a = 0;
  let b = chars.length;
  const trim = () => {
    while (a < b && !isWordChar(chars[a]!) && !APOSTROPHES.has(chars[a]!)) a += 1;
    while (b > a && !isWordChar(chars[b - 1]!) && !APOSTROPHES.has(chars[b - 1]!)) b -= 1;
  };
  trim();
  // An edge apostrophe is a glottal stop or ejective mark (`'ayo`, `ka'`) unless the word is wrapped in a quote pair.
  const openedByQuote = a > 0 && chars[a - 1] === "\u2018";
  if (openedByQuote && b - a > 1 && QUOTE_CLOSE.has(chars[b - 1]!)) {
    b -= 1;
    trim();
  } else if (b - a > 2 && chars[a] === "'" && QUOTE_CLOSE.has(chars[b - 1]!)) {
    a += 1;
    b -= 1;
    trim();
  }
  return { lead: chars.slice(0, a).join(""), core: chars.slice(a, b).join(""), trail: chars.slice(b).join("") };
}

/** The words of a transcription, in order. Punctuation-only chunks join the word before them (or the one after, at the start). */
export function tokenize(content: string): Word[] {
  const words: Word[] = [];
  let pendingLead = "";
  for (const chunk of content.split(/[\s  -   　]+/u)) {
    if (chunk === "") continue;
    if (!hasSubstance(chunk)) {
      const last = words[words.length - 1];
      if (last !== undefined) last.trail += chunk;
      else pendingLead += chunk;
      continue;
    }
    const { lead, core, trail } = split(chunk);
    words.push({ form: core.normalize("NFC"), lead: pendingLead + lead, trail });
    pendingLead = "";
  }
  return words;
}

/** Just the forms: what `tokenize` hands the engine. */
export const forms = (content: string): string[] => tokenize(content).map((w) => w.form);
