// Personal pronouns for the grammar sketch: the lexicon's pronoun entries
// placed in a person × number table by their glosses (1SG, 3PL, 1PL.INCL,
// or the English pronoun a gloss is). Pure.

export type PronounEntry = { lemma: string; category: string; gloss: string };
export type Person = "1" | "1in" | "1ex" | "2" | "3";
export type GNumber = "SG" | "DU" | "PL" | "";
export type Placed = { person: Person; number: GNumber };

/** Whether a part of speech names pronouns (pro, pron, pronoun, PRO, pers. pron.). */
export const isPronoun = (category: string): boolean => /^(pro(n(oun)?)?|pers(onal)?[ .]*pro(n(oun)?)?|personal)\b/iu.test(category.trim());

const ENGLISH: Record<string, Placed> = {
  i: { person: "1", number: "SG" },
  me: { person: "1", number: "SG" },
  we: { person: "1", number: "PL" },
  us: { person: "1", number: "PL" },
  you: { person: "2", number: "" },
  "you (sg)": { person: "2", number: "SG" },
  "you (pl)": { person: "2", number: "PL" },
  "you all": { person: "2", number: "PL" },
  "y'all": { person: "2", number: "PL" },
  he: { person: "3", number: "SG" },
  she: { person: "3", number: "SG" },
  it: { person: "3", number: "SG" },
  him: { person: "3", number: "SG" },
  her: { person: "3", number: "SG" },
  "he/she": { person: "3", number: "SG" },
  "s/he": { person: "3", number: "SG" },
  they: { person: "3", number: "PL" },
  them: { person: "3", number: "PL" },
  "we two": { person: "1", number: "DU" },
  "you two": { person: "2", number: "DU" },
  "they two": { person: "3", number: "DU" },
};

/** A pronoun's person and number from its gloss, or null when the gloss says neither. */
export function personNumber(gloss: string): Placed | null {
  const g = gloss.trim().toLowerCase().replace(/\s+/gu, " ");
  if (g === "") return null;
  const english = ENGLISH[g] ?? ENGLISH[g.split(/[;,]/u)[0]!.trim()];
  if (english !== undefined) return english;
  const parts = g.split(/[.\s;:/_-]+/u).filter((p) => p !== "");
  let person: Person | null = null;
  let number: GNumber = "";
  for (const p of parts) {
    const m = /^([123])(sg|s|du|d|pl|p)?$/u.exec(p);
    if (m !== null) {
      person = m[1] as "1" | "2" | "3";
      if (m[2] !== undefined) number = m[2].startsWith("s") ? "SG" : m[2].startsWith("d") ? "DU" : "PL";
    } else if (p === "sg") number = "SG";
    else if (p === "du") number = "DU";
    else if (p === "pl") number = "PL";
  }
  if (person === null) return null;
  if (person === "1" && parts.some((p) => p === "incl" || p === "inc" || p === "in")) person = "1in";
  if (person === "1" && parts.some((p) => p === "excl" || p === "exc" || p === "ex")) person = "1ex";
  return { person, number };
}

const PERSONS: { id: Person; label: string }[] = [
  { id: "1", label: "1" },
  { id: "1in", label: "1 inclusive" },
  { id: "1ex", label: "1 exclusive" },
  { id: "2", label: "2" },
  { id: "3", label: "3" },
];
const NUMBERS: { id: GNumber; label: string }[] = [
  { id: "SG", label: "Singular" },
  { id: "DU", label: "Dual" },
  { id: "PL", label: "Plural" },
  { id: "", label: "Number not glossed" },
];

export type PronounTable = { rows: string[]; columns: string[]; cells: string[][][]; unplaced: string[] };

/** The pronoun entries in a person × number table, only the persons and numbers met; entries whose gloss says no person listed apart. */
export function pronounTable(entries: readonly PronounEntry[]): PronounTable | null {
  const pronouns = entries.filter((e) => isPronoun(e.category));
  if (pronouns.length === 0) return null;
  const placed = pronouns.map((e) => ({ e, at: personNumber(e.gloss) }));
  const persons = PERSONS.filter((p) => placed.some((x) => x.at?.person === p.id));
  const numbers = NUMBERS.filter((n) => placed.some((x) => x.at !== null && x.at.number === n.id));
  const cells = persons.map((p) => numbers.map((n) => placed.filter((x) => x.at?.person === p.id && x.at.number === n.id).map((x) => x.e.lemma)));
  return { rows: persons.map((p) => p.label), columns: numbers.map((n) => n.label), cells, unplaced: placed.filter((x) => x.at === null).map((x) => `${x.e.lemma} ‘${x.e.gloss}’`) };
}
