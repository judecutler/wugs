// Pure helpers for the Lexicon panel (roadmap item 22). No DOM.

/** Fold for matching: NFC, lower case, diacritics kept apart from letters. */
const fold = (s: string): string => s.normalize("NFC").toLocaleLowerCase();
/** Fold that also drops diacritics, so "kopek" finds "köpek". */
const loose = (s: string): string => fold(s).normalize("NFD").replace(/\p{M}/gu, "");

/** "yüz₂" for a numbered homograph, the bare lemma otherwise. */
export function headword(lemma: string, homograph: number | null): string {
  if (homograph === null) return lemma;
  const sub = "₀₁₂₃₄₅₆₇₈₉";
  return lemma + String(homograph).replace(/\d/g, (d) => sub[Number(d)] ?? d);
}

/** One line under the headword: the senses' glosses, numbered when there are several. */
export function glossLine(glosses: string[]): string {
  const shown = glosses.map((g) => g.trim()).filter((g) => g !== "");
  if (shown.length <= 1) return shown[0] ?? "";
  return shown.map((g, i) => `${i + 1}. ${g}`).join("  ");
}

export type DomainLike = { code: string; label: string };

/** Does a domain match what was typed? A leading digit searches the outline
 * number ("2.1" finds 2.1 and everything under it); words search the label,
 * every word must appear, diacritics and case folded. */
export function domainMatches(d: DomainLike, query: string): boolean {
  const q = query.trim();
  if (q === "") return true;
  if (/^\d/.test(q)) return d.code === q || d.code.startsWith(q.endsWith(".") ? q : q + ".");
  const label = loose(d.label);
  return loose(q).split(/\s+/).every((w) => label.includes(w));
}

/** At most `limit` matching domains, in the list's own (tree) order. */
export function filterDomains<T extends DomainLike>(all: T[], query: string, limit = 50): T[] {
  const out: T[] = [];
  for (const d of all) {
    if (domainMatches(d, query)) out.push(d);
    if (out.length >= limit) break;
  }
  return out;
}

/** A homograph number as typed: "" clears it; otherwise a whole number ≥ 1. */
export function parseHomograph(input: string): { ok: true; value: number | null } | { ok: false } {
  const t = input.trim();
  if (t === "") return { ok: true, value: null };
  if (!/^\d+$/.test(t)) return { ok: false };
  const n = Number(t);
  return n >= 1 && n <= 999 ? { ok: true, value: n } : { ok: false };
}

/** The next free homograph number among entries sharing a lemma. */
export function nextHomograph(taken: (number | null)[]): number {
  let n = 1;
  while (taken.includes(n)) n += 1;
  return n;
}

/** The notice shown when other entries share this lemma, or null. */
export function sameLemmaNote(lemma: string, own: number | null, others: (number | null)[]): string | null {
  if (others.length === 0) return null;
  const count = others.length === 1 ? "Another entry" : `${others.length} other entries`;
  const unnumbered = own === null || others.some((h) => h === null);
  return unnumbered
    ? `${count} also spelled «${lemma}». Number them as homographs (1, 2, …) to tell them apart.`
    : `${count} also spelled «${lemma}»; told apart by homograph number.`;
}

/** "used in 3 words" / "not used in any text yet". */
export function usageLabel(n: number): string {
  if (n === 0) return "not used in any text yet";
  return n === 1 ? "used in 1 word" : `used in ${n} words`;
}
