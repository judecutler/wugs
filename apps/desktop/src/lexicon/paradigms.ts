// Paradigm grids for the Lexicon panel (roadmap item 50). No DOM.
// TEMPLATES is the one table of built-in grids: the panel reads it, and the
// engine is told only the features and values to lay out (it knows no
// templates). Values are lower-case Leipzig abbreviations.

export type FeatureValue = { value: string; label: string };
export type Axis = { feature: string; label: string; values: FeatureValue[] };
/** `cols` null: a single column (singular and plural). */
export type Template = { id: string; label: string; rows: Axis; cols: Axis | null };

const PERSON: Axis = {
  feature: "person",
  label: "Person",
  values: [
    { value: "1", label: "1st person" },
    { value: "2", label: "2nd person" },
    { value: "3", label: "3rd person" },
  ],
};
const NUMBER: Axis = {
  feature: "number",
  label: "Number",
  values: [
    { value: "sg", label: "singular" },
    { value: "pl", label: "plural" },
  ],
};
const TENSE: Axis = {
  feature: "tense",
  label: "Tense",
  values: [
    { value: "prs", label: "present" },
    { value: "pst", label: "past" },
    { value: "fut", label: "future" },
  ],
};

export const TEMPLATES: readonly Template[] = [
  { id: "person-number", label: "Person × number", rows: PERSON, cols: NUMBER },
  { id: "tense-person", label: "Tense × person", rows: TENSE, cols: PERSON },
  { id: "number", label: "Singular and plural", rows: NUMBER, cols: null },
];

/** The template with this id among the built-in ones and the project's
 * own (`own`, from `paradigm_templates`), else the first built-in one. */
export function templateById(id: string | null, own: readonly Template[] = []): Template {
  return TEMPLATES.find((t) => t.id === id) ?? own.find((t) => t.id === id) ?? TEMPLATES[0]!;
}

/** Whether a template is one of the project's own (editable), not built in. */
export function isOwn(t: Template): boolean {
  return !TEMPLATES.some((b) => b.id === t.id);
}

/** An axis's values as typed in the template form: "nom=nominative, acc,
 * gen=genitive" (the value, then optionally what it means). */
export function valuesText(a: Axis): string {
  return a.values.map((v) => (v.label === v.value ? v.value : `${v.value}=${v.label}`)).join(", ");
}

/** The values typed in the form, in order; an empty piece is dropped. */
export function parseValues(text: string): FeatureValue[] {
  return text
    .split(/[,;\n]/)
    .map((piece) => {
      const at = piece.indexOf("=");
      const value = (at < 0 ? piece : piece.slice(0, at)).trim();
      const label = at < 0 ? value : piece.slice(at + 1).trim() || value;
      return { value, label };
    })
    .filter((v) => v.value !== "");
}

/** A feature name the engine takes (letters, digits and hyphens), made from
 * what was typed: "Noun class" becomes "noun-class". */
export function featureName(typed: string): string {
  return typed
    .trim()
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 35);
}

/** A new template's id from its name, not one already taken. */
export function templateId(name: string, taken: readonly string[]): string {
  const base = featureName(name).slice(0, 34) || "template";
  let id = base;
  for (let n = 2; taken.includes(id) || TEMPLATES.some((t) => t.id === id); n++) id = `${base}-${n}`;
  return id;
}

export type TemplateDraft = { name: string; rows: string; rowValues: string; cols: string; colValues: string };

/** A draft filled in from a template, for editing it. */
export function draftOf(t: Template | null): TemplateDraft {
  if (t === null) return { name: "", rows: "", rowValues: "", cols: "", colValues: "" };
  return { name: t.label, rows: t.rows.label, rowValues: valuesText(t.rows), cols: t.cols?.label ?? "", colValues: t.cols === null ? "" : valuesText(t.cols) };
}

/** The template the form describes, or what is missing in words. `id` keeps
 * an edited template's id; a new one gets one from its name. */
export function templateFromDraft(d: TemplateDraft, id: string | null, taken: readonly string[]): Template | string {
  const name = d.name.trim();
  if (name === "") return "Give the template a name.";
  const axis = (label: string, values: string, what: string): Axis | string => {
    const feature = featureName(label);
    if (feature === "") return `Name the feature the ${what} stand for (case, number, noun class…).`;
    const vs = parseValues(values);
    if (vs.length === 0) return `List the ${what}' values, separated by commas.`;
    if (vs.length > 16) return `At most 16 values in the ${what}.`;
    const seen = new Set<string>();
    for (const v of vs) {
      const k = v.value.toLowerCase();
      if (seen.has(k)) return `«${v.value}» is listed twice in the ${what}.`;
      seen.add(k);
    }
    return { feature, label: label.trim(), values: vs };
  };
  const rows = axis(d.rows, d.rowValues, "rows");
  if (typeof rows === "string") return rows;
  let cols: Axis | null = null;
  if (d.cols.trim() !== "" || d.colValues.trim() !== "") {
    const c = axis(d.cols, d.colValues, "columns");
    if (typeof c === "string") return c;
    if (c.feature === rows.feature) return "The rows and the columns need two different features.";
    cols = c;
  }
  return { id: id ?? templateId(name, taken), label: name, rows, cols };
}

/** What the paradigm commands take: the template's features and values, rows first. */
export function axesOf(t: Template): { feature: string; values: string[] }[] {
  return [t.rows, ...(t.cols === null ? [] : [t.cols])].map((a) => ({ feature: a.feature, values: a.values.map((v) => v.value) }));
}

export type Features = Record<string, string>;
export type FormLike = { id: string; form: string; features: Features };
export type CellLike<F extends FormLike = FormLike> = { features: Features; forms: F[] };

/** The features of the cell at a row value and (when the template has columns) a column value. */
export function cellFeatures(t: Template, row: string, col: string | null): Features {
  const f: Features = { [t.rows.feature]: row };
  if (t.cols !== null && col !== null) f[t.cols.feature] = col;
  return f;
}

function sameFeatures(a: Features, b: Features): boolean {
  const ka = Object.keys(a);
  return ka.length === Object.keys(b).length && ka.every((k) => a[k] === b[k]);
}

/** The grid as rows of cells, found by features, so a paradigm answered for
 * another template (an edit still in flight when the template changed)
 * shows nothing rather than forms in the wrong cells. */
export function layout<C extends CellLike>(t: Template, cells: C[]): (C | null)[][] {
  const cols = t.cols === null ? [null] : t.cols.values.map((v) => v.value);
  return t.rows.values.map((r) => cols.map((c) => cells.find((cell) => sameFeatures(cell.features, cellFeatures(t, r.value, c))) ?? null));
}

/** "4 of 6 cells filled", for the cells of this template the paradigm holds. */
export function coverageLine(t: Template, cells: CellLike[]): string {
  const grid = layout(t, cells).flat();
  const filled = grid.filter((c) => c !== null && c.forms.length > 0).length;
  return `${filled} of ${grid.length} cells filled`;
}

/** A cell's name in words, for screen readers and titles: "1st person singular". */
export function cellLabel(t: Template, features: Features): string {
  const word = (a: Axis) => a.values.find((v) => v.value === features[a.feature])?.label ?? features[a.feature] ?? "";
  return [word(t.rows), t.cols === null ? "" : word(t.cols)].filter((w) => w !== "").join(" ");
}

/** A form that fits no cell, with its features by name: "gitti (person 3, tense pst)". */
export function otherFormLine(f: FormLike): string {
  const feats = Object.keys(f.features)
    .sort()
    .map((k) => `${k} ${f.features[k]}`);
  return feats.length === 0 ? `${f.form} (no features)` : `${f.form} (${feats.join(", ")})`;
}

/** A stable focus key for the i-th input of a cell: the same before and
 * after typing into an empty cell creates its form, so focus stays put. */
export function cellKey(lexeme: string, features: Features, i: number): string {
  const f = Object.keys(features)
    .sort()
    .map((k) => `${k}=${features[k]}`)
    .join(",");
  return `pdg:${lexeme}:${f}:${i}`;
}

/** One word form of the entry met in the texts: as written, how it was
 * analysed, and how often. */
export type Attested = { form: string; breakdown: string | null; glosses: string | null; count: number; inGrid: boolean };

/** The entry's word forms in the texts, from its senses' concordance lines:
 * grouped by form and analysis (case ignored), commonest first, each marked
 * when the paradigm already holds that form. */
export function attestedForms(
  lines: readonly { words: string[]; at: number; breakdown: string | null; glosses: string | null }[],
  paradigmForms: readonly string[],
): Attested[] {
  const fold = (s: string) => s.normalize("NFC").toLocaleLowerCase();
  const held = new Set(paradigmForms.map(fold));
  const found = new Map<string, Attested & { spellings: Map<string, number> }>();
  for (const l of lines) {
    const form = l.words[l.at];
    if (form === undefined) continue;
    const key = `${fold(form)}\u001f${l.breakdown ?? ""}\u001f${l.glosses ?? ""}`;
    const a = found.get(key) ?? { form, breakdown: l.breakdown, glosses: l.glosses, count: 0, inGrid: held.has(fold(form)), spellings: new Map() };
    a.count += 1;
    a.spellings.set(form, (a.spellings.get(form) ?? 0) + 1);
    found.set(key, a);
  }
  return [...found.values()]
    .map(({ spellings, ...a }) => ({ ...a, form: [...spellings].sort((x, y) => y[1] - x[1])[0]![0] }))
    .sort((a, b) => b.count - a.count || a.form.localeCompare(b.form));
}

/** The line above the attested forms. */
export function attestedLine(forms: readonly Attested[], lines: number, total: number): string {
  if (forms.length === 0) return "No word in the texts is linked to this entry yet. Gloss a few and link them to it.";
  const missing = forms.filter((f) => !f.inGrid).length;
  const kinds = forms.length === 1 ? "1 form" : `${forms.length} forms`;
  const more = total > lines ? ` (from the first ${lines} of ${total} uses)` : "";
  const gap = missing === 0 ? " Every one is in the paradigm." : missing === forms.length ? " None is in the paradigm yet." : ` ${missing} not in the paradigm yet.`;
  return `${kinds} of this entry in the texts${more}.${gap}`;
}

/** A gloss line's grammatical parts in upper case, person digits apart:
 * `1SG-count-PST` gives 1, SG, PST. Lower-case meanings are left out. */
export function grammaticalParts(glosses: string): Set<string> {
  const out = new Set<string>();
  for (const piece of glosses.split(/[\s\-=~.:;<>]+/u)) {
    if (piece === "" || /\p{Ll}/u.test(piece)) continue;
    const m = /^(\d)?(.*)$/u.exec(piece)!;
    if (m[1] !== undefined) out.add(m[1]);
    if (m[2] !== "") out.add(m[2]!.toUpperCase());
  }
  return out;
}

/** The template cell a word's glosses name: one value of each axis among its
 * grammatical parts, and only one (a gloss with both SG and PL names none). */
export function cellOf(t: Template, glosses: string | null): Features | null {
  if (glosses === null) return null;
  const parts = grammaticalParts(glosses);
  const pick = (a: Axis): string | null => {
    const hit = a.values.filter((v) => parts.has(v.value.toUpperCase()));
    return hit.length === 1 ? hit[0]!.value : null;
  };
  const row = pick(t.rows);
  if (row === null) return null;
  if (t.cols === null) return cellFeatures(t, row, null);
  const col = pick(t.cols);
  return col === null ? null : cellFeatures(t, row, col);
}

/** Attested forms that can go into an empty cell of the grid: not in the
 * paradigm yet, their glosses naming one cell, that cell empty, and the
 * commonest form when two name the same cell. */
export function fillable<C extends CellLike>(t: Template, cells: C[], forms: readonly Attested[]): { form: string; features: Features }[] {
  const taken = new Set(cells.filter((c) => c.forms.length > 0).map((c) => JSON.stringify(Object.entries(c.features).sort())));
  const out: { form: string; features: Features }[] = [];
  for (const f of forms) {
    if (f.inGrid) continue;
    const features = cellOf(t, f.glosses);
    if (features === null) continue;
    const key = JSON.stringify(Object.entries(features).sort());
    if (taken.has(key)) continue;
    taken.add(key);
    out.push({ form: f.form, features });
  }
  return out;
}

/** The paradigm as a table for a paper, in the format Copy example uses:
 * an HTML table (with the same as tab-separated text beside it) for Word
 * and Google Docs, a LaTeX `tabular`, or columns lined up with spaces. */
export function paradigmClipboard<C extends CellLike>(t: Template, cells: C[], headword: string, format: "word" | "gb4e" | "expex" | "text"): { text: string; html: string | null } {
  const grid = layout(t, cells);
  const head = ["", ...(t.cols === null ? ["form"] : t.cols.values.map((v) => v.label))];
  const body = t.rows.values.map((r, i) => [r.label, ...grid[i]!.map((c) => (c === null || c.forms.length === 0 ? "—" : c.forms.map((f) => f.form).join(", ")))]);
  const tsv = [head, ...body].map((row) => row.join("\t")).join("\n") + "\n";
  if (format === "word") {
    const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    const th = (s: string) => `<th style="text-align:left;padding:2px 10px;border-bottom:1px solid #000">${esc(s)}</th>`;
    const html = `<table style="border-collapse:collapse"><caption style="text-align:left"><i>${esc(headword)}</i></caption><tr>${head.map(th).join("")}</tr>${body
      .map((row) => `<tr>${row.map((c, j) => (j === 0 ? `<td style="padding:2px 10px">${esc(c)}</td>` : `<td style="padding:2px 10px"><i>${esc(c)}</i></td>`)).join("")}</tr>`)
      .join("")}</table>`;
    return { text: tsv, html };
  }
  if (format === "gb4e" || format === "expex") {
    const tex = (s: string) => s.replace(/\\/g, "\\textbackslash{}").replace(/([&%$#_{}])/g, "\\$1").replace(/~/g, "\\textasciitilde{}").replace(/\^/g, "\\textasciicircum{}");
    const lines = [
      `% ${tex(headword)}`,
      `\\begin{tabular}{l${"l".repeat(head.length - 1)}}`,
      `${head.map(tex).join(" & ")} \\\\ \\hline`,
      ...body.map((row) => `${row.map((c, j) => (j === 0 ? tex(c) : `\\textit{${tex(c)}}`)).join(" & ")} \\\\`),
      "\\end{tabular}",
    ];
    return { text: lines.join("\n") + "\n", html: null };
  }
  const units = new Intl.Segmenter(undefined, { granularity: "grapheme" });
  const width = (s: string) => [...units.segment(s)].filter((g) => !/^\p{M}+$/u.test(g.segment)).length;
  const rows = [head, ...body];
  const widths = head.map((_, j) => Math.max(...rows.map((r) => width(r[j]!))));
  const text = [headword, ...rows.map((r) => r.map((c, j) => c + " ".repeat(widths[j]! - width(c))).join("  ").trimEnd())].join("\n") + "\n";
  return { text, html: null };
}

const escHtml = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** A paradigm as a printable form for a session without a laptop: a header
 * with lines for the speaker, date and place, the grid with the forms
 * already known in grey (or none, for a blind check) and room to write. */
export function paradigmSheet<C extends CellLike>(t: Template, cells: C[], headword: string, gloss: string, withForms: boolean): string {
  const grid = layout(t, cells);
  const cols = t.cols === null ? [null] : t.cols.values;
  const field = (label: string) => `<div class="field"><span>${escHtml(label)}</span><span class="line"></span></div>`;
  const head = t.cols === null ? "" : `<tr><th></th>${t.cols.values.map((v) => `<th>${escHtml(v.label)}</th>`).join("")}</tr>`;
  const rows = t.rows.values
    .map((r, ri) => {
      const tds = cols.map((_, ci) => {
        const c = grid[ri]?.[ci] ?? null;
        const known = withForms && c !== null && c.forms.length > 0 ? `<span class="had">${c.forms.map((f) => escHtml(f.form)).join(", ")}</span>` : "";
        return `<td>${known}</td>`;
      });
      return `<tr><th>${escHtml(r.label)}</th>${tds.join("")}</tr>`;
    })
    .join("\n");
  const title = `${headword}${gloss.trim() ? ` ‘${gloss.trim()}’` : ""}: ${t.label}`;
  const style = `@page{margin:15mm}body{font-family:"Charis SIL","Gentium Plus","Doulos SIL",serif;font-size:11pt;margin:0 auto;max-width:180mm}
h1{font-size:15pt;margin:0 0 8pt}.head{display:grid;grid-template-columns:1fr 1fr;gap:4pt 16pt;margin-bottom:12pt}.field{display:flex;gap:6pt;align-items:flex-end}.field span:first-child{white-space:nowrap}.line{flex:1;border-bottom:1px solid #000;height:14pt}
table{border-collapse:collapse;width:100%}th,td{border:1px solid #777;padding:4pt 6pt;text-align:left;vertical-align:top}th{font-size:9.5pt;background:#f2f2f2}td{height:34pt}.had{color:#888}
.notes{margin-top:14pt}.notes .line{display:block;height:18pt}.foot{color:#777;font-size:8pt;margin-top:8pt}`;
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>${escHtml(title)}</title>
<style>${style}</style>
</head>
<body>
<h1>${escHtml(title)}</h1>
<div class="head">${field("Speaker")}${field("Date")}${field("Place")}${field("Recording")}</div>
<table>
${head}
${rows}
</table>
<div class="notes"><div>Notes</div><span class="line"></span><span class="line"></span><span class="line"></span></div>
<p class="foot">Printed from wugs. ${withForms ? "Forms in grey are already in the lexicon; check them with the speaker." : "Every cell left blank, for a fresh check."}</p>
</body>
</html>
`;
}
