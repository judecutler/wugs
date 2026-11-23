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
