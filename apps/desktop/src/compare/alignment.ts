// One concept's words aligned in columns, the project's sounds across the
// top (a star alignment: each other form aligned with the project's form by
// `align`), as EDICTOR shows a cognate set. A sound with nothing across from
// it in the project's word joins the next cell (the last one at the end), so
// every column stays one of the project's sounds. Pure; npm-tested.

import { judgementOf, type Judgements } from "./cognates.ts";
import { align, GAP } from "./correspondences.ts";
import { CLOSE, type Row } from "./logic.ts";

export type AlignedRow = { language: string; form: string; judged: boolean; cells: string[] };
export type Alignment = { ours: string; columns: string[]; rows: AlignedRow[] };

/** The forms of a concept to align: those judged cognate, else (with
 * `close`) those at least `CLOSE` alike to the project's word. */
export function cognateForms(r: Row, languages: readonly string[], judged: Judgements, close: boolean): { language: string; form: string; judged: boolean }[] {
  const out: { language: string; form: string; judged: boolean }[] = [];
  for (const language of languages) {
    for (const c of r.others.get(language) ?? []) {
      const j = judgementOf(judged, r, language, c.form);
      if (j === "y") out.push({ language, form: c.form, judged: true });
      else if (j === undefined && close && (c.similarity ?? 0) >= CLOSE) out.push({ language, form: c.form, judged: false });
    }
  }
  return out;
}

/** Every form set against the project's word, column by column. */
export function alignAll(ours: string, forms: readonly { language: string; form: string; judged: boolean }[]): Alignment {
  const columns = align(ours, ours).map(([o]) => o);
  const rows = forms.map((f) => {
    const cells: string[] = [];
    let extra = "";
    for (const [o, t] of align(ours, f.form)) {
      if (o === GAP) extra += t;
      else {
        cells.push(extra === "" ? t : t === GAP ? extra : extra + t);
        extra = "";
      }
    }
    if (extra !== "" && cells.length > 0) cells[cells.length - 1] = cells[cells.length - 1] === GAP ? extra : cells[cells.length - 1] + extra;
    return { language: f.language, form: f.form, judged: f.judged, cells };
  });
  return { ours, columns, rows };
}

/** How many forms keep the project's sound in each column. */
export function kept(a: Alignment): number[] {
  return a.columns.map((c, i) => a.rows.filter((r) => r.cells[i] === c).length);
}

/** The alignment as a table: a row per language, a column per sound. */
export function alignmentTsv(concept: string, a: Alignment): string {
  const head = ["language", "form", ...a.columns].join("\t");
  const ours = ["(this project)", a.ours, ...a.columns].join("\t");
  const rows = a.rows.map((r) => [r.language, r.form, ...r.cells].join("\t"));
  return [`# ${concept}`, head, ours, ...rows].join("\n") + "\n";
}

/** The line under the table. */
export function alignmentLine(a: Alignment, judged: number): string {
  if (a.rows.length === 0) return "No form of another language is judged cognate with this word yet; tick the close ones to align them too.";
  const k = kept(a);
  const stable = a.columns.filter((_, i) => k[i] === a.rows.length).length;
  return `${a.rows.length} ${a.rows.length === 1 ? "form" : "forms"} (${judged} judged cognate) set against «${a.ours}»: ${stable} of its ${a.columns.length} sounds the same in every one.`;
}
