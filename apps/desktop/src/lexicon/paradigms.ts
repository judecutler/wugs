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

/** The template with this id, else the first. */
export function templateById(id: string | null): Template {
  return TEMPLATES.find((t) => t.id === id) ?? TEMPLATES[0]!;
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
