// Near (AntConc's context words, CQPweb's collocation filter): keep the
// Concordance lines with another word within a few words of the match.
import { type Hit } from "./logic.ts";

export type NearSide = "either" | "before" | "after";

export const NEAR_SIDES: { id: NearSide; label: string }[] = [
  { id: "either", label: "before or after" },
  { id: "before", label: "before" },
  { id: "after", label: "after" },
];

export const NEAR_WINDOWS = [1, 2, 3, 5, 10] as const;

/** How Near compares words: NFC, case folded; marks kept, as the Concordance keeps them. */
export const nearKey = (word: string): string => word.normalize("NFC").toLocaleLowerCase().replace(/^[\p{P}\p{S}]+|[\p{P}\p{S}]+$/gu, "");

/** A test for one word: `*` any letters, `?` one, case ignored; an empty pattern matches nothing. */
export function nearTest(pattern: string): (word: string) => boolean {
  // The pattern keeps its * and ?; only words lose punctuation at their edges.
  const p = pattern.trim().normalize("NFC").toLocaleLowerCase();
  if (/^\**$/u.test(p)) return () => false;
  const re = new RegExp(`^${[...p].map((c) => (c === "*" ? ".*" : c === "?" ? "." : c.replace(/[.+^${}()|[\]\\]/gu, "\\$&"))).join("")}$`, "u");
  return (word) => re.test(nearKey(word));
}

/** The lines with a word matching `pattern` within `window` words of the match, on `side`. */
export function nearHits(hits: readonly Hit[], pattern: string, window: number, side: NearSide): Hit[] {
  const test = nearTest(pattern);
  return hits.filter((h) => {
    const end = h.at + Math.max(1, h.len ?? 1);
    const before = side === "after" ? [] : h.words.slice(Math.max(0, h.at - window), h.at);
    const after = side === "before" ? [] : h.words.slice(end, end + window);
    return before.some(test) || after.some(test);
  });
}

/** Said after the count line while Near is on. */
export function nearLine(kept: number, of: number, pattern: string, window: number, side: NearSide): string {
  const sides = side === "either" ? "before or after" : side;
  const where = window === 1 ? `right ${sides} it` : `within ${window} words ${sides}`;
  const lines = (n: number) => (n === 1 ? "1 line" : `${n.toLocaleString("en")} lines`);
  const who = kept === of ? "Every line has" : `${lines(kept)} of ${of.toLocaleString("en")} ${kept === 1 ? "has" : "have"}`;
  return ` ${who} «${pattern.trim()}» ${where}.`;
}
