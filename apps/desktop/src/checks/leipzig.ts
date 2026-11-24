// Glosses against the Leipzig Glossing Rules: what a journal's copy editor
// would mark before the examples go to print, with the fix to make it once
// for every morph glossed that way (src-tauri/src/consistency.rs `regloss`).
// Pure: no DOM.
import { LEIPZIG } from "../gloss/abbreviations.ts";

/** One gloss as written, how many morphs carry it, how many of them are affixes or clitics, and where it first occurs. */
export type GlossForm = { gloss: string; count: number; affixes: number; textId: string; textTitle: string; tierId: string; segmentId: string };
export type LeipzigIssue = { form: GlossForm; to: string; reasons: string[] };

/** Leipzig's one-word meanings, each to its abbreviation (`plural` → PL, `negative` → NEG). */
const WORD_TO_ABBR: ReadonlyMap<string, string> = new Map(
  Object.entries(LEIPZIG).flatMap(([abbr, meaning]) =>
    meaning
      .split(/,\s*/u)
      .filter((w) => /^\p{Ll}+$/u.test(w))
      .map((w) => [w, abbr] as [string, string]),
  ),
);

const PERSON = /^([123])(sg|du|pl)$/iu;
/** A word longer than this is a description, not a gloss: leave it be. */
const MAX_WORDS = 4;

export const MARKS_REASON = "Boundary marks (- = ~) go between the morphs, not inside a gloss.";
export const WORDS_REASON = "Rule 4: a gloss of several words joins them with full stops (go.out).";
export const CAPS_REASON = "Rule 3: grammatical categories are abbreviated in capitals, set as small capitals (PL, PST).";

/** A gloss part an affix carries, written as Leipzig abbreviates it, or as it was. */
function grammatical(part: string): string {
  if (part === "" || /\p{Lu}/u.test(part) && part === part.toUpperCase()) return part;
  const upper = part.toUpperCase();
  const person = PERSON.exec(part);
  if (person !== null) return upper;
  if (part.length > 1 && LEIPZIG[upper] !== undefined && /^\p{L}+$/u.test(part)) return upper;
  return WORD_TO_ABBR.get(part.toLowerCase()) ?? part;
}

/** What the rules would change in one gloss, and why; null when nothing. */
export function leipzigFix(form: GlossForm): LeipzigIssue | null {
  let g = form.gloss.trim();
  const reasons: string[] = [];
  const bare = g.replace(/^[-=~]+|[-=~]+$/gu, "");
  if (bare !== g && bare !== "") {
    g = bare;
    reasons.push(MARKS_REASON);
  }
  const words = g.split(/\s+/u).filter(Boolean);
  if (words.length > 1 && words.length <= MAX_WORDS) {
    g = words.join(".");
    reasons.push(WORDS_REASON);
  }
  // Only a gloss carried mostly by affixes: a root glossed «past» means the past.
  if (form.affixes * 2 >= form.count && form.affixes > 0 && !/\s/u.test(g)) {
    const caps = g
      .split(".")
      .map(grammatical)
      .join(".");
    if (caps !== g) {
      g = caps;
      reasons.push(CAPS_REASON);
    }
  }
  return g === form.gloss.trim() ? null : { form, to: g, reasons };
}

/** Every gloss the rules would change, the most used first. */
export function leipzigIssues(forms: readonly GlossForm[]): LeipzigIssue[] {
  return forms
    .map(leipzigFix)
    .filter((x): x is LeipzigIssue => x !== null)
    .sort((a, b) => b.form.count - a.form.count || a.form.gloss.localeCompare(b.form.gloss));
}

/** The line above the cards. */
export function leipzigLine(issues: readonly LeipzigIssue[], forms: number): string {
  if (forms === 0) return "No glosses yet.";
  if (issues.length === 0) return `All ${forms.toLocaleString("en")} glosses follow the Leipzig Glossing Rules as far as wugs can tell.`;
  const uses = issues.reduce((n, i) => n + i.form.count, 0);
  return `${issues.length.toLocaleString("en")} ${issues.length === 1 ? "gloss" : "glosses"} (${uses.toLocaleString("en")} ${uses === 1 ? "morph" : "morphs"}) could follow the Leipzig Glossing Rules more closely, as journals expect of examples.`;
}
