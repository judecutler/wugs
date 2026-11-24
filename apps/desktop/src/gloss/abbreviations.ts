// The abbreviations a set of glossed examples uses, for the list every
// grammar and paper prints before its examples. Meanings come from the
// project (what its linguists wrote), else the Leipzig Glossing Rules'
// standard list; an abbreviation with neither is listed for the author to
// explain, which also catches a mistyped one (`PLU` beside `PL`). Pure: no DOM.
import { glossRuns, type Example } from "./example.ts";

/** The Leipzig Glossing Rules' list of standard abbreviations (Appendix). */
export const LEIPZIG: Readonly<Record<string, string>> = {
  "1": "first person",
  "2": "second person",
  "3": "third person",
  A: "agent-like argument of canonical transitive verb",
  ABL: "ablative",
  ABS: "absolutive",
  ACC: "accusative",
  ADJ: "adjective",
  ADV: "adverb(ial)",
  AGR: "agreement",
  ALL: "allative",
  ANTIP: "antipassive",
  APPL: "applicative",
  ART: "article",
  AUX: "auxiliary",
  BEN: "benefactive",
  CAUS: "causative",
  CLF: "classifier",
  COM: "comitative",
  COMP: "complementizer",
  COMPL: "completive",
  COND: "conditional",
  COP: "copula",
  CVB: "converb",
  DAT: "dative",
  DECL: "declarative",
  DEF: "definite",
  DEM: "demonstrative",
  DET: "determiner",
  DIST: "distal",
  DISTR: "distributive",
  DU: "dual",
  DUR: "durative",
  ERG: "ergative",
  EXCL: "exclusive",
  F: "feminine",
  FOC: "focus",
  FUT: "future",
  GEN: "genitive",
  IMP: "imperative",
  INCL: "inclusive",
  IND: "indicative",
  INDF: "indefinite",
  INF: "infinitive",
  INS: "instrumental",
  INTR: "intransitive",
  IPFV: "imperfective",
  IRR: "irrealis",
  LOC: "locative",
  M: "masculine",
  N: "neuter",
  NEG: "negation, negative",
  NMLZ: "nominalizer/nominalization",
  NOM: "nominative",
  OBJ: "object",
  OBL: "oblique",
  P: "patient-like argument of canonical transitive verb",
  PASS: "passive",
  PFV: "perfective",
  PL: "plural",
  POSS: "possessive",
  PRED: "predicative",
  PRF: "perfect",
  PRS: "present",
  PROG: "progressive",
  PROH: "prohibitive",
  PROX: "proximal/proximate",
  PST: "past",
  PTCP: "participle",
  PURP: "purposive",
  Q: "question particle/marker",
  QUOT: "quotative",
  RECP: "reciprocal",
  REFL: "reflexive",
  REL: "relative",
  RES: "resultative",
  S: "single argument of canonical intransitive verb",
  SBJ: "subject",
  SBJV: "subjunctive",
  SG: "singular",
  TOP: "topic",
  TR: "transitive",
  VOC: "vocative",
};

/** The Leipzig meaning of `abbr`, reading `N` before a listed one as "non-"
 * (`NSG` nonsingular, `NPST` nonpast), as the rules' list does. */
export function leipzigMeaning(abbr: string): string | null {
  const own = LEIPZIG[abbr];
  if (own !== undefined) return own;
  if (abbr.length > 1 && abbr.startsWith("N")) {
    const rest = LEIPZIG[abbr.slice(1)];
    if (rest !== undefined && !/^\d/u.test(abbr.slice(1))) return `non${rest}`;
  }
  return null;
}

/** The abbreviations in one morph's gloss: its grammatical runs as the
 * examples set them in small capitals (`glossRuns`), with person digits taken
 * apart from what follows (`3SG` is `3` and `SG`). Lexical parts (`dog` in
 * `dog.PL`) are not abbreviations. */
export function abbreviationsIn(gloss: string): string[] {
  const out: string[] = [];
  for (const run of glossRuns(gloss)) {
    if (!run.caps) continue;
    for (const piece of run.text.match(/\p{N}|[^\p{N}]+/gu) ?? []) out.push(piece);
  }
  return out;
}

/** Every abbreviation the examples use, once each: person numbers first,
 * then in alphabetical order. */
export function abbreviationsOf(examples: readonly Example[]): string[] {
  const seen = new Set<string>();
  for (const ex of examples) for (const w of ex.words) for (const m of w.morphs) for (const a of abbreviationsIn(m.gloss ?? "")) seen.add(a);
  return sortAbbreviations([...seen]);
}

export function sortAbbreviations(list: readonly string[]): string[] {
  const digit = (a: string) => /^\p{N}+$/u.test(a);
  return [...list].sort((a, b) => Number(digit(b)) - Number(digit(a)) || (digit(a) ? Number(a) - Number(b) : 0) || (a < b ? -1 : a > b ? 1 : 0));
}

/** One line of the list: whose meaning it shows. */
export type Abbreviation = { abbr: string; meaning: string | null; source: "project" | "leipzig" | "none" };

/** The list for `used`, the project's own meanings winning over Leipzig's. */
export function abbreviationList(used: readonly string[], own: ReadonlyMap<string, string>): Abbreviation[] {
  return used.map((abbr) => {
    const mine = own.get(abbr)?.trim() ?? "";
    if (mine !== "") return { abbr, meaning: mine, source: "project" };
    const l = leipzigMeaning(abbr);
    return l === null ? { abbr, meaning: null, source: "none" } : { abbr, meaning: l, source: "leipzig" };
  });
}

/** The note under the list in the Interlinear panel. */
export function unexplainedNote(list: readonly Abbreviation[]): string | null {
  const missing = list.filter((a) => a.source === "none").map((a) => a.abbr);
  if (missing.length === 0) return null;
  const names = missing.length > 3 ? `${missing.slice(0, 3).join(", ")} and ${missing.length - 3} more` : missing.join(", ");
  return `${names} ${missing.length === 1 ? "is" : "are"} not in the Leipzig list: write ${missing.length === 1 ? "its meaning" : "their meanings"} below, or check for a typing slip.`;
}

/** What the grammatical abbreviations of a gloss being typed mean, for a hint
 * under the cell: `3SG.PST` gives "3 third person · SG singular · PST past".
 * The project's own meanings win; one with no meaning is marked "?" so a slip
 * shows at once. Empty when the gloss has no abbreviation. */
export function glossHint(gloss: string, own: ReadonlyMap<string, string | null>): string {
  const seen = new Set<string>();
  const parts: string[] = [];
  for (const a of abbreviationsIn(gloss)) {
    if (seen.has(a)) continue;
    seen.add(a);
    const meaning = own.get(a)?.trim() || leipzigMeaning(a);
    parts.push(meaning ? `${a} ${meaning}` : `${a} ?`);
  }
  return parts.join(" · ");
}
