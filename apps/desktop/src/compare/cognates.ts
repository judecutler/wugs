// Cognate judgements in Compare word lists: for each other language's form,
// whether it is cognate with the project's word for the concept (y), not
// (n), or not yet judged; kept with the project as a table
// (src-tauri/src/cognates.rs). From them, the share of judged concepts
// that are cognate per language (lexicostatistics), and cognate sets for
// the EDICTOR file. Pure, npm-tested.

import { conceptKey, edictorTsv, type Row } from "./logic.ts";

export type Judgement = "y" | "n";
export type Judgements = ReadonlyMap<string, Judgement>;

/** A judgement's key: the other language, the project's concept, the form. */
export const judgeKey = (language: string, concept: string, form: string): string => [language.trim(), conceptKey(concept), form.normalize("NFC").trim()].join("\t");

/** The kept table read back; lines that do not read are skipped. */
export function parseJudgements(text: string): Map<string, Judgement> {
  const out = new Map<string, Judgement>();
  for (const line of text.split("\n")) {
    const [v, language, concept, form] = line.split("\t");
    if ((v === "y" || v === "n") && language && concept && form) out.set(judgeKey(language, concept, form), v);
  }
  return out;
}

/** The table to keep: a line per judgement. */
export const judgementsText = (j: Judgements): string =>
  [...j]
    .map(([k, v]) => `${v}\t${k}`)
    .sort()
    .join("\n");

/** What a click does: not judged → cognate → not cognate → not judged. */
export const nextJudgement = (j: Judgement | undefined): Judgement | undefined => (j === undefined ? "y" : j === "y" ? "n" : undefined);

export const judgementOf = (j: Judgements, r: Row, language: string, form: string): Judgement | undefined => j.get(judgeKey(language, r.concept.gloss, form));

export type Cognacy = { language: string; judged: number; cognate: number };

/** Per language, the concepts the project answers whose forms are judged
 * (any judgement) and how many of them have a form judged cognate. */
export function cognacy(rows: readonly Row[], languages: readonly string[], j: Judgements): Cognacy[] {
  return languages.map((language) => {
    let judged = 0;
    let cognate = 0;
    for (const r of rows) {
      if (r.ours.length === 0) continue;
      const marks = (r.others.get(language) ?? []).map((c) => judgementOf(j, r, language, c.form)).filter((x) => x !== undefined);
      if (marks.length === 0) continue;
      judged += 1;
      if (marks.includes("y")) cognate += 1;
    }
    return { language, judged, cognate };
  });
}

/** In words: "Teke: 31 of 40 judged concepts cognate (78%)". */
export function cognacyLine(c: Cognacy): string {
  if (c.judged === 0) return `${c.language}: nothing judged yet.`;
  return `${c.language}: ${c.cognate} of ${c.judged} judged ${c.judged === 1 ? "concept" : "concepts"} cognate (${Math.round((100 * c.cognate) / c.judged)}%).`;
}

/** The EDICTOR file with a COGID column: per concept, the project's forms
 * and every form judged cognate share one set; every other form is a set of
 * its own. */
export function edictorWithCognates(rows: readonly Row[], ourLanguage: string, j: Judgements): string {
  let next = 0;
  const shared = new Map<Row, number>();
  return edictorTsv(rows, ourLanguage, (r, language, form) => {
    const set = () => {
      const s = shared.get(r) ?? ++next;
      shared.set(r, s);
      return s;
    };
    if (language === null) return set();
    return judgementOf(j, r, language, form) === "y" ? set() : ++next;
  });
}
