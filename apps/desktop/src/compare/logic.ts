// Compare word lists: the project's answers to a concept list beside other
// languages' words for the same concepts (read by src-tauri/src/compare.rs),
// with how alike each pair is, and the whole table as an EDICTOR/LingPy
// file for cognate judgements there. Pure; npm-tested.

import { sounds } from "../sounds/ipa.ts";

export type OtherForm = { language: string; concept: string; form: string };
export type OtherList = { name: string; forms: OtherForm[]; columns: string[]; skipped: number };
/** A concept of the project's list with its answers (ConceptView, narrowed). */
export type OurConcept = { id: string; number: number; gloss: string; concepticonGloss: string; answers: { lemma: string }[] };

export type Cell = { form: string; similarity: number | null };
export type Row = { concept: OurConcept; ours: string[]; others: Map<string, Cell[]> };

/** How a concept is matched across lists: case, accents and spacing aside. */
export const conceptKey = (s: string): string =>
  s
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .replace(/[_\s]+/gu, " ")
    .trim();

/** Edit distance over sounds (a letter with its marks one unit), as a share
 * of the longer form: 1 the same, 0 nothing alike. Affix markers are left out. */
export function similarity(a: string, b: string): number {
  const x = sounds(a.replace(/[-=]/gu, ""));
  const y = sounds(b.replace(/[-=]/gu, ""));
  if (x.length === 0 && y.length === 0) return 1;
  let prev = Array.from({ length: y.length + 1 }, (_, j) => j);
  for (let i = 1; i <= x.length; i++) {
    const cur = [i];
    for (let j = 1; j <= y.length; j++) cur[j] = Math.min(prev[j]! + 1, cur[j - 1]! + 1, prev[j - 1]! + (x[i - 1] === y[j - 1] ? 0 : 1));
    prev = cur;
  }
  return 1 - prev[y.length]! / Math.max(x.length, y.length);
}

/** Every language in the loaded lists, in the order first met. */
export const languagesOf = (forms: readonly OtherForm[]): string[] => [...new Set(forms.map((f) => f.language))];

/** One row per concept of the project's list: its answers, and each other
 * language's forms for it (matched by gloss or Concepticon gloss), each
 * with its best similarity to one of the answers. */
export function compareRows(concepts: readonly OurConcept[], forms: readonly OtherForm[]): Row[] {
  const byKey = new Map<string, OtherForm[]>();
  for (const f of forms) {
    const k = conceptKey(f.concept);
    const list = byKey.get(k) ?? [];
    list.push(f);
    byKey.set(k, list);
  }
  return concepts.map((concept) => {
    const ours = [...new Set(concept.answers.map((a) => a.lemma))];
    const keys = new Set([conceptKey(concept.gloss), conceptKey(concept.concepticonGloss)].filter((k) => k !== ""));
    const others = new Map<string, Cell[]>();
    for (const k of keys)
      for (const f of byKey.get(k) ?? []) {
        const cells = others.get(f.language) ?? [];
        if (!cells.some((c) => c.form === f.form)) cells.push({ form: f.form, similarity: ours.length === 0 ? null : Math.max(...ours.map((o) => similarity(o, f.form))) });
        others.set(f.language, cells);
      }
    return { concept, ours, others };
  });
}

/** A similarity worth marking: about two sounds in three alike. */
export const CLOSE = 0.6;

/** The line over the table. */
export function compareLine(rows: readonly Row[], languages: readonly string[], forms: readonly OtherForm[]): string {
  if (languages.length === 0) return "Load another language's word list (a CSV or tab-separated table with a concept and a form column, an EDICTOR file or a CLDF forms.csv) to set it beside your answers.";
  const matched = new Set(rows.flatMap((r) => [...r.others.values()].flat().map((c) => c.form)));
  const keys = new Set(rows.flatMap((r) => [conceptKey(r.concept.gloss), conceptKey(r.concept.concepticonGloss)]));
  const unmatched = new Set(forms.filter((f) => !keys.has(conceptKey(f.concept))).map((f) => conceptKey(f.concept))).size;
  const close = rows.reduce((n, r) => n + [...r.others.values()].flat().filter((c) => (c.similarity ?? 0) >= CLOSE).length, 0);
  const langs = `${languages.length} ${languages.length === 1 ? "language" : "languages"}`;
  return `${langs} beside your answers, ${matched.size} forms matched to a concept; ${close} look alike (two sounds in three or more).${unmatched > 0 ? ` ${unmatched} of their concepts are not in this list.` : ""} Alike is not cognate: judge them in EDICTOR or by hand.`;
}

/** The whole table as an EDICTOR/LingPy wordlist: the project's answers under its own language, then each other language's forms. */
export function edictorTsv(rows: readonly Row[], ourLanguage: string): string {
  const out = ["ID\tDOCULECT\tCONCEPT\tFORM\tTOKENS"];
  let id = 0;
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  const add = (lang: string, concept: string, form: string) => {
    id += 1;
    out.push([id, clean(lang), clean(concept), clean(form), sounds(form.replace(/[-=]/gu, "")).join(" ")].join("\t"));
  };
  for (const r of rows) {
    const concept = r.concept.concepticonGloss || r.concept.gloss;
    for (const f of r.ours) add(ourLanguage || "This project", concept, f);
    for (const [lang, cells] of r.others) for (const c of cells) add(lang, concept, c.form);
  }
  return out.join("\n") + "\n";
}
