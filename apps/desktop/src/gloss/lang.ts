// The language tag of a second gloss row (`gloss:<tag>`). Pure: no DOM.
// The engine accepts letters, digits and hyphens, 2 to 35 characters, and
// keeps the tag lowercase, so the panel offers exactly that.

/** The tag as the engine stores it, or null when it is not a usable tag. */
export function normalizeLangTag(input: string): string | null {
  const tag = input.trim().toLowerCase();
  return /^[a-z0-9-]{2,35}$/.test(tag) ? tag : null;
}
