// A glossed segment as a numbered example for a paper (roadmap 69): the step
// linguists otherwise do by hand from FLEx or Toolbox. Pure: no DOM.
// Lines follow the Leipzig Glossing Rules: the morph line and the gloss line
// carry the same boundary markers, grammatical glosses are small capitals,
// and the free translation is in single quotes.
import { isGrammatical } from "./leipzig.ts";
import { joinedBreakdown, type MorphType } from "./morphs.ts";

export type ExampleWord = { form: string; morphs: { surface: string; type: string; gloss: string | null }[] };
/** `source`: where the example comes from (`Heole 2010: 12`), written after the translation in parentheses. */
export type Example = { words: ExampleWord[]; translation: string | null; number: number; source?: string };

export type ExampleFormat = "word" | "gb4e" | "expex" | "text";

export const EXAMPLE_FORMATS: { id: ExampleFormat; label: string; note: string }[] = [
  { id: "word", label: "Word / Google Docs", note: "A table: paste into Word, Google Docs or LibreOffice" },
  { id: "gb4e", label: "LaTeX (gb4e)", note: "\\gll lines for the gb4e package" },
  { id: "expex", label: "LaTeX (ExPex)", note: "\\begingl lines for the ExPex package" },
  { id: "text", label: "Plain text", note: "Lines lined up with spaces, for email or notes" },
];

export const FORMAT_KEY = "wugs.gloss.copyFormat";
/** Whether copied examples name their source (localStorage, "on" or not). */
export const SOURCE_KEY = "wugs.gloss.copySource";

export function parseFormat(stored: string | null): ExampleFormat {
  return EXAMPLE_FORMATS.some((f) => f.id === stored) ? (stored as ExampleFormat) : "word";
}

/** A gloss the linguist has not filled in shows as `?`, so the gap is visible in the paper. */
const MISSING = "?";

/** One word's two lines: morphs joined at their markers, and glosses joined the same way. */
export function wordLines(w: ExampleWord): { morphs: string; glosses: string } {
  if (w.morphs.length === 0) return { morphs: w.form, glosses: "" };
  const type = (t: string) => t as MorphType;
  const glossOf = (m: { gloss: string | null }) => (m.gloss === null || m.gloss.trim() === "" ? MISSING : m.gloss.trim());
  // An infix inside a root is stored as the root's two halves around it
  // (`s<um>ulat`). The root is glossed once, before the infix (Leipzig rule 9:
  // `write<AV>`), unless its second half carries a gloss of its own.
  const glossed = w.morphs.filter((m, i) => {
    const before = w.morphs[i - 1];
    const root = w.morphs[i - 2];
    const secondHalf = m.type === "root" && before?.type === "infix" && root?.type === "root";
    return !(secondHalf && (glossOf(m) === MISSING || glossOf(m) === glossOf(root)));
  });
  return {
    morphs: joinedBreakdown(w.morphs.map((m) => ({ surface: m.surface, type: type(m.type) }))),
    glosses: joinedBreakdown(glossed.map((m) => ({ surface: glossOf(m), type: type(m.type) }))),
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

/** The translation in quotes and the source after it: the example's last line. */
function lastLine(ex: Example): string {
  const src = (ex.source ?? "").trim();
  return [quoted(ex.translation), src === "" ? "" : `(${src})`].filter((x) => x !== "").join(" ");
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
  const t = lastLine(ex);
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
  // Under pdflatex's default encoding a bare < or > prints as ¡ or ¿.
  "<": "\\textless{}",
  ">": "\\textgreater{}",
};
export const latexEscape = (s: string): string => s.replace(/[\\{}&%$#_~^<>]/gu, (c) => LATEX_SPECIAL[c] ?? c);

/** gb4e and ExPex split a line into columns at spaces, so a word or gloss
 * holding one is grouped. gb4e takes braces as they are; ExPex drops the
 * braces of a group that stands alone (`{go out}` printed "go") and loses
 * the space inside one (`{a b}` printed "ab"), so for ExPex the space is
 * written `\ ` as well. Both checked by compiling with XeLaTeX. */
type Flavour = "gb4e" | "expex";
const grouped = (set: string, spaced: boolean, flavour: Flavour): string => (!spaced ? set : flavour === "expex" ? `{${set.replace(/\s+/gu, "\\ ")}}` : `{${set}}`);

/** A gloss for LaTeX: grammatical runs as \textsc{lowercase}, the rest escaped. */
function latexGloss(gloss: string, flavour: Flavour): string {
  if (gloss === "") return "{}";
  const set = glossRuns(gloss)
    .map((r) => (r.caps ? `\\textsc{${latexEscape(r.text.toLowerCase())}}` : latexEscape(r.text)))
    .join("");
  return grouped(set, /\s/u.test(gloss), flavour);
}

const latexWord = (s: string, flavour: Flavour): string => (s === "" ? "{}" : grouped(latexEscape(s), /\s/u.test(s), flavour));

/** The translation in TeX quotes and the source after it, escaped. */
function latexTail(ex: Example): string {
  const t = (ex.translation ?? "").trim();
  const src = (ex.source ?? "").trim();
  return [t === "" ? "" : `\`${latexEscape(t)}'`, src === "" ? "" : `(${latexEscape(src)})`].filter((x) => x !== "").join(" ");
}

export function asGb4e(ex: Example): string {
  const lines = ex.words.map(wordLines);
  const out = [
    "\\begin{exe}",
    `\\ex \\gll ${lines.map((l) => latexWord(l.morphs, "gb4e")).join(" ")}\\\\`,
    `     ${lines.map((l) => latexGloss(l.glosses, "gb4e")).join(" ")}\\\\`,
  ];
  const t = latexTail(ex);
  if (t !== "") out.push(`\\glt ${t}`);
  out.push("\\end{exe}");
  return out.join("\n");
}

export function asExpex(ex: Example): string {
  const lines = ex.words.map(wordLines);
  const t = latexTail(ex);
  return [
    "\\ex",
    "\\begingl",
    `\\gla ${lines.map((l) => latexWord(l.morphs, "expex")).join(" ")}//`,
    `\\glb ${lines.map((l) => latexGloss(l.glosses, "expex")).join(" ")}//`,
    ...(t === "" ? [] : [`\\glft ${t}//`]),
    "\\endgl",
    "\\xe",
  ].join("\n");
}

const htmlEscape = (s: string): string => s.replace(/[&<>"]/gu, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] ?? c);

function htmlGloss(gloss: string): string {
  return glossRuns(gloss)
    // Capitals at 80%: reads as small capitals in Word, Google Docs (which has
    // no small-caps setting) and LibreOffice alike, and stays a capital gloss.
    .map((r) => (r.caps ? `<span style="font-size: 80%">${htmlEscape(r.text)}</span>` : htmlEscape(r.text)))
    .join("");
}

/** A borderless table, one column per word, the way word processors set an
 * interlinear example; the number sits in its own first column. */
export function asHtml(ex: Example): string {
  const lines = ex.words.map(wordLines);
  const cell = (inner: string, extra = "") => `<td style="padding: 0 0.8em 0 0; border: none; vertical-align: top${extra}">${inner}</td>`;
  const morphRow = lines.map((l) => cell(`<i>${htmlEscape(l.morphs)}</i>`)).join("");
  const glossRow = lines.map((l) => cell(htmlGloss(l.glosses))).join("");
  const t = lastLine(ex);
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

/** Several examples on the clipboard at once (the Concordance's lines as a
 * paper's examples): one after another, a blank line apart in text, a
 * paragraph apart in HTML. */
export function clipboardForAll(examples: readonly Example[], format: ExampleFormat): { text: string; html: string | null } {
  const parts = examples.map((ex) => clipboardFor(ex, format));
  const html = format === "word" ? parts.map((p) => p.html ?? "").join("<p></p>") : null;
  return { text: parts.map((p) => p.text.trimEnd()).join("\n\n") + "\n", html };
}
