// A glossed segment as a numbered example for a paper (roadmap 69): the step
// linguists otherwise do by hand from FLEx or Toolbox. Pure: no DOM.
// Lines follow the Leipzig Glossing Rules: the morph line and the gloss line
// carry the same boundary markers, grammatical glosses are small capitals,
// and the free translation is in single quotes.
import { isGrammatical } from "./leipzig.ts";
import { joinedBreakdown, type MorphType } from "./morphs.ts";

export type ExampleWord = { form: string; morphs: { surface: string; type: string; gloss: string | null }[] };
export type Example = { words: ExampleWord[]; translation: string | null; number: number };

export type ExampleFormat = "word" | "gb4e" | "expex" | "text";

export const EXAMPLE_FORMATS: { id: ExampleFormat; label: string; note: string }[] = [
  { id: "word", label: "Word / Google Docs", note: "A table: paste into Word, Google Docs or LibreOffice" },
  { id: "gb4e", label: "LaTeX (gb4e)", note: "\\gll lines for the gb4e package" },
  { id: "expex", label: "LaTeX (ExPex)", note: "\\begingl lines for the ExPex package" },
  { id: "text", label: "Plain text", note: "Lines lined up with spaces, for email or notes" },
];

export const FORMAT_KEY = "wugs.gloss.copyFormat";

export function parseFormat(stored: string | null): ExampleFormat {
  return EXAMPLE_FORMATS.some((f) => f.id === stored) ? (stored as ExampleFormat) : "word";
}

/** A gloss the linguist has not filled in shows as `?`, so the gap is visible in the paper. */
const MISSING = "?";

/** One word's two lines: morphs joined at their markers, and glosses joined the same way. */
export function wordLines(w: ExampleWord): { morphs: string; glosses: string } {
  if (w.morphs.length === 0) return { morphs: w.form, glosses: "" };
  const type = (t: string) => t as MorphType;
  return {
    morphs: joinedBreakdown(w.morphs.map((m) => ({ surface: m.surface, type: type(m.type) }))),
    glosses: joinedBreakdown(w.morphs.map((m) => ({ surface: m.gloss === null || m.gloss.trim() === "" ? MISSING : m.gloss.trim(), type: type(m.type) }))),
  };
}

/** Grammatical runs of a gloss, so each format can set them in small capitals.
 * Splits on the Leipzig separators and keeps them. */
export function glossRuns(gloss: string): { text: string; caps: boolean }[] {
  const runs: { text: string; caps: boolean }[] = [];
  for (const piece of gloss.split(/([-=~.<>:])/u)) {
    if (piece === "") continue;
    runs.push({ text: piece, caps: isGrammatical(piece) });
  }
  return runs;
}

/** How many columns a string takes in a monospace font: grapheme clusters, so
 * combining diacritics (common in IPA) do not push the columns apart. */
export function width(s: string): number {
  const Seg = (Intl as unknown as { Segmenter?: new (l?: string, o?: { granularity: string }) => { segment(s: string): Iterable<unknown> } }).Segmenter;
  if (Seg !== undefined) return [...new Seg(undefined, { granularity: "grapheme" }).segment(s)].length;
  return [...s.normalize("NFC")].length;
}

const pad = (s: string, n: number) => s + " ".repeat(Math.max(0, n - width(s)));

function quoted(translation: string | null): string {
  const t = (translation ?? "").trim();
  return t === "" ? "" : `‘${t}’`;
}

export function asText(ex: Example): string {
  const lines = ex.words.map(wordLines);
  const cols = lines.map((l) => Math.max(width(l.morphs), width(l.glosses)));
  const row = (pick: (l: { morphs: string; glosses: string }) => string) =>
    lines
      .map((l, i) => pad(pick(l), cols[i] ?? 0))
      .join("  ")
      .trimEnd();
  const label = `(${ex.number})`;
  const indent = " ".repeat(label.length + 1);
  const out = [`${label} ${row((l) => l.morphs)}`, `${indent}${row((l) => l.glosses)}`];
  const t = quoted(ex.translation);
  if (t !== "") out.push(`${indent}${t}`);
  return out.join("\n");
}

const LATEX_SPECIAL: Record<string, string> = {
  "\\": "\\textbackslash{}",
  "{": "\\{",
  "}": "\\}",
  "&": "\\&",
  "%": "\\%",
  $: "\\$",
  "#": "\\#",
  _: "\\_",
  "~": "\\textasciitilde{}",
  "^": "\\textasciicircum{}",
};
export const latexEscape = (s: string): string => s.replace(/[\\{}&%$#_~^]/gu, (c) => LATEX_SPECIAL[c] ?? c);

/** A gloss for LaTeX: grammatical runs as \textsc{lowercase}, the rest escaped. */
function latexGloss(gloss: string): string {
  if (gloss === "") return "{}";
  return glossRuns(gloss)
    .map((r) => (r.caps ? `\\textsc{${latexEscape(r.text.toLowerCase())}}` : latexEscape(r.text)))
    .join("");
}

/** gb4e and ExPex split words on spaces, so a word containing one is braced. */
const latexWord = (s: string): string => (s === "" ? "{}" : /\s/u.test(s) ? `{${latexEscape(s)}}` : latexEscape(s));

export function asGb4e(ex: Example): string {
  const lines = ex.words.map(wordLines);
  const out = [
    "\\begin{exe}",
    `\\ex \\gll ${lines.map((l) => latexWord(l.morphs)).join(" ")}\\\\`,
    `     ${lines.map((l) => latexGloss(l.glosses)).join(" ")}\\\\`,
  ];
  const t = (ex.translation ?? "").trim();
  out.push(`\\glt \`${latexEscape(t)}'`, "\\end{exe}");
  return out.join("\n");
}

export function asExpex(ex: Example): string {
  const lines = ex.words.map(wordLines);
  const t = (ex.translation ?? "").trim();
  return [
    "\\ex",
    "\\begingl",
    `\\gla ${lines.map((l) => latexWord(l.morphs)).join(" ")}//`,
    `\\glb ${lines.map((l) => latexGloss(l.glosses)).join(" ")}//`,
    `\\glft \`${latexEscape(t)}'//`,
    "\\endgl",
    "\\xe",
  ].join("\n");
}

const htmlEscape = (s: string): string => s.replace(/[&<>"]/gu, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] ?? c);

function htmlGloss(gloss: string): string {
  return glossRuns(gloss)
    .map((r) => (r.caps ? `<span style="font-variant: small-caps">${htmlEscape(r.text.toLowerCase())}</span>` : htmlEscape(r.text)))
    .join("");
}

/** A borderless table, one column per word, the way word processors set an
 * interlinear example; the number sits in its own first column. */
export function asHtml(ex: Example): string {
  const lines = ex.words.map(wordLines);
  const cell = (inner: string, extra = "") => `<td style="padding: 0 0.8em 0 0; border: none; vertical-align: top${extra}">${inner}</td>`;
  const morphRow = lines.map((l) => cell(`<i>${htmlEscape(l.morphs)}</i>`)).join("");
  const glossRow = lines.map((l) => cell(htmlGloss(l.glosses))).join("");
  const t = quoted(ex.translation);
  const span = lines.length;
  return [
    '<table style="border-collapse: collapse; border: none">',
    `<tr>${cell(`(${ex.number})`)}${morphRow}</tr>`,
    `<tr>${cell("")}${glossRow}</tr>`,
    t === "" ? "" : `<tr>${cell("")}<td colspan="${span}" style="border: none">${htmlEscape(t)}</td></tr>`,
    "</table>",
  ].join("");
}

/** What goes on the clipboard: always plain text, and HTML for word processors. */
export function clipboardFor(ex: Example, format: ExampleFormat): { text: string; html: string | null } {
  switch (format) {
    case "word":
      return { text: asText(ex), html: asHtml(ex) };
    case "gb4e":
      return { text: asGb4e(ex), html: null };
    case "expex":
      return { text: asExpex(ex), html: null };
    case "text":
      return { text: asText(ex), html: null };
  }
}
