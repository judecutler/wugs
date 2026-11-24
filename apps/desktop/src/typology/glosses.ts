// Grambank answers the glossed affixes speak for (the Affixes panel's
// "Grambank from the glosses"): a plural affix on nouns says GB044 is
// present, a past-tense affix on verbs says GB083, and so on. Only
// presence is ever suggested: a category no affix is glossed with may still
// exist, unglossed or not yet met. Pure; no DOM.
import { marked, type Affix } from "../affixes/logic.ts";

/** Uses needed before a feature is suggested, so one stray gloss says nothing. */
export const MIN_USES = 3;

type Class = "noun" | "pronoun" | "verb";

const CLASSES: Record<Class, ReadonlySet<string>> = {
  noun: new Set(["n", "noun", "nn", "nom", "nominal", "substantive", "sustantivo", "nomen", "propn", "n.prop", "prop", "proper noun"]),
  pronoun: new Set(["pro", "pron", "pronoun", "prn", "pers", "personal pronoun", "pers.pro", "ppron"]),
  verb: new Set(["v", "verb", "vb", "vi", "vt", "v.i", "v.t", "vi.", "vt.", "verbo", "intransitive verb", "transitive verb", "aux", "v.aux"]),
};

/** The class of a root category as people write it (case and edge dots ignored). */
export function classOf(category: string): Class | null {
  const c = category.trim().toLowerCase();
  for (const k of Object.keys(CLASSES) as Class[]) if (CLASSES[k].has(c) || CLASSES[k].has(c.replace(/\.+$/u, ""))) return k;
  return null;
}

/** The grammatical tokens of a gloss: `3SG.SBJ-PST` → 3SG, SBJ, PST. Lexical
 * (lower-case) parts are left out. */
export function glossTokens(gloss: string): string[] {
  return gloss
    .split(/[.:\-=_\s]+/u)
    .filter((t) => t !== "" && t === t.toUpperCase() && /[A-Z0-9]/u.test(t));
}

const isPerson = (t: string) => /^[123]/u.test(t);
const CORE = new Set(["NOM", "ACC", "ERG", "ABS"]);
const OBLIQUE = new Set(["DAT", "GEN", "LOC", "ABL", "ALL", "INS", "INST", "INSTR", "COM", "COMIT", "BEN", "PERL", "ELA", "ILL", "INE", "ADE", "ESS", "TRANSL", "ABE", "SOC", "LAT", "TERM", "PROL"]);
/** Tokens that only index an argument: person, number, gender, role. */
const AGREEMENT = new Set(["SG", "PL", "DU", "PAU", "S", "A", "P", "O", "SBJ", "SUBJ", "OBJ", "M", "F", "N", "EXCL", "INCL", "HUM", "NHUM"]);

/** An affix's uses on roots of one class (root counts by category). */
function on(a: Affix, cls: Class): boolean {
  return a.categories.some(([c, n]) => n > 0 && classOf(c) === cls);
}

const has = (a: Affix, ...wanted: string[]) => glossTokens(a.gloss).some((t) => wanted.includes(t));
const before = (a: Affix) => a.slot < 0 || a.kind === "prefix" || a.kind === "proclitic";
const after = (a: Affix) => !before(a) && a.kind !== "infix";
/** Indexes an argument: a person in the gloss, and every token agreement. */
const personOnly = (a: Affix) => {
  const t = glossTokens(a.gloss);
  return t.length > 0 && t.some(isPerson) && t.every((x) => isPerson(x) || AGREEMENT.has(x));
};
/** The roles a person marker names outright (`3SG.SBJ`, `1SG.A`, `2SG.OBJ`). */
function roles(a: Affix): Set<"S" | "A" | "P"> {
  const t = glossTokens(a.gloss);
  const out = new Set<"S" | "A" | "P">();
  if (!t.some(isPerson)) return out;
  for (const x of t) {
    if (x === "S") out.add("S");
    if (x === "A") out.add("A");
    if (x === "P" || x === "O" || x === "OBJ") out.add("P");
    if (x === "SBJ" || x === "SUBJ") {
      out.add("S");
      out.add("A");
    }
  }
  return out;
}

type Rule = { id: string; says: string; test: (a: Affix) => boolean };

/** Each feature with what counts as evidence for it. */
export const RULES: readonly Rule[] = [
  { id: "GB042", says: "a singular affix on nouns", test: (a) => on(a, "noun") && has(a, "SG") },
  { id: "GB043", says: "a dual affix on nouns", test: (a) => on(a, "noun") && has(a, "DU") },
  { id: "GB044", says: "a plural affix on nouns", test: (a) => on(a, "noun") && has(a, "PL") },
  { id: "GB070", says: "a core case (NOM, ACC, ERG, ABS) on nouns", test: (a) => on(a, "noun") && glossTokens(a.gloss).some((t) => CORE.has(t)) },
  { id: "GB071", says: "a core case on pronouns", test: (a) => on(a, "pronoun") && glossTokens(a.gloss).some((t) => CORE.has(t)) },
  { id: "GB072", says: "an oblique case (DAT, GEN, LOC…) on nouns", test: (a) => on(a, "noun") && glossTokens(a.gloss).some((t) => OBLIQUE.has(t)) },
  { id: "GB073", says: "an oblique case on pronouns", test: (a) => on(a, "pronoun") && glossTokens(a.gloss).some((t) => OBLIQUE.has(t)) },
  { id: "GB079", says: "a prefix or proclitic on verbs that does more than index an argument", test: (a) => on(a, "verb") && before(a) && a.kind !== "infix" && !personOnly(a) && glossTokens(a.gloss).length > 0 },
  { id: "GB080", says: "a suffix or enclitic on verbs that does more than index an argument", test: (a) => on(a, "verb") && after(a) && !personOnly(a) && glossTokens(a.gloss).length > 0 },
  { id: "GB081", says: "an infix in verbs", test: (a) => on(a, "verb") && a.kind === "infix" },
  { id: "GB082", says: "a present-tense affix on verbs", test: (a) => on(a, "verb") && has(a, "PRS", "PRES") },
  { id: "GB083", says: "a past-tense affix on verbs", test: (a) => on(a, "verb") && has(a, "PST", "PAST") },
  { id: "GB084", says: "a future-tense affix on verbs", test: (a) => on(a, "verb") && has(a, "FUT") },
  { id: "GB086", says: "a perfective or imperfective affix on verbs", test: (a) => on(a, "verb") && has(a, "PFV", "IPFV", "IMPF", "PERF") },
  { id: "GB089", says: "an S-indexing suffix on verbs", test: (a) => on(a, "verb") && after(a) && roles(a).has("S") },
  { id: "GB090", says: "an S-indexing prefix on verbs", test: (a) => on(a, "verb") && before(a) && roles(a).has("S") },
  { id: "GB091", says: "an A-indexing suffix on verbs", test: (a) => on(a, "verb") && after(a) && roles(a).has("A") },
  { id: "GB092", says: "an A-indexing prefix on verbs", test: (a) => on(a, "verb") && before(a) && roles(a).has("A") },
  { id: "GB093", says: "a P-indexing suffix on verbs", test: (a) => on(a, "verb") && after(a) && roles(a).has("P") },
  { id: "GB094", says: "a P-indexing prefix on verbs", test: (a) => on(a, "verb") && before(a) && roles(a).has("P") },
  { id: "GB107", says: "a negative affix on verbs", test: (a) => on(a, "verb") && has(a, "NEG") },
];

export type GlossEvidence = { id: string; says: string; affixes: Affix[]; uses: number };

/** The features the glossed affixes give evidence of, in Grambank's order,
 * each with its affixes (most used first) and their uses. */
export function glossEvidence(affixes: readonly Affix[]): GlossEvidence[] {
  const out: GlossEvidence[] = [];
  for (const r of RULES) {
    const found = affixes.filter(r.test).sort((a, b) => b.uses - a.uses);
    const uses = found.reduce((n, a) => n + a.uses, 0);
    if (uses >= MIN_USES) out.push({ id: r.id, says: r.says, affixes: found, uses });
  }
  return out;
}

/** The evidence in words, kept as the answer's note: «From the glossed
 * affixes: -lar PL (120 uses), -ler PL (80 uses).» */
export function evidenceNote(e: GlossEvidence, at = 4): string {
  const shown = e.affixes.slice(0, at).map((a) => `${marked(a)} ${a.gloss} (${a.uses} ${a.uses === 1 ? "use" : "uses"})`);
  const more = e.affixes.length > at ? `, and ${e.affixes.length - at} more` : "";
  return `From the glossed affixes: ${shown.join(", ")}${more}.`;
}

/** The line above the list. */
export function evidenceLine(found: number, open: number): string {
  if (found === 0) return `No Grambank feature is shown by the glossed affixes yet. An affix glossed with a Leipzig abbreviation (PL, PST, ERG…) on roots whose entries have a category (n, v, pro) counts once it is used ${MIN_USES} times.`;
  const n = (x: number, one: string, many: string) => `${x} ${x === 1 ? one : many}`;
  const answered = open === 0 ? " All are answered present in this project already." : ` ${open} of them ${open === 1 ? "is" : "are"} not yet answered present in this project.`;
  return `The glossed affixes show ${n(found, "Grambank feature", "Grambank features")} present.${answered} Only presence is read from the glosses: a feature not listed may still be present.`;
}
