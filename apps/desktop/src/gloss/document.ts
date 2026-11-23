// A whole text as numbered interlinear examples, for a grammar, a paper or
// a handout (roadmap 43, publishing): the same builders as Copy example, so
// an example reads the same pasted alone or in the document. Pure: no DOM.
import { asExpex, asGb4e, asHtml, asText, latexEscape, type Example, type ExampleFormat } from "./example.ts";

export type Document = { extension: "tex" | "html" | "txt"; contents: string };

const htmlEscape = (s: string): string => s.replace(/[&<>"]/gu, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] ?? c);

/** The examples as one file in the chosen format. LaTeX files compile with
 * XeLaTeX or LuaLaTeX, which read IPA and every script as typed. */
export function textDocument(title: string, examples: readonly Example[], format: ExampleFormat): Document {
  switch (format) {
    case "expex":
    case "gb4e": {
      const pkg = format === "expex" ? "\\usepackage{expex}" : "\\usepackage{gb4e}\n\\noautomath";
      const body = examples.map((ex) => `% (${ex.number})\n${format === "expex" ? asExpex(ex) : asGb4e(ex)}`).join("\n\n");
      return {
        extension: "tex",
        contents: [
          "% Exported from wugs. Compile with XeLaTeX or LuaLaTeX (they read IPA and every script);",
          "% change the font below to one your computer has, such as Charis SIL, Doulos SIL or Gentium.",
          "\\documentclass{article}",
          "\\usepackage{fontspec}",
          "\\setmainfont{Charis SIL}",
          pkg,
          "\\begin{document}",
          `\\section*{${latexEscape(title)}}`,
          "",
          body,
          "",
          "\\end{document}",
          "",
        ].join("\n"),
      };
    }
    case "word":
      return {
        extension: "html",
        contents: [
          "<!DOCTYPE html>",
          '<html><head><meta charset="utf-8">',
          `<title>${htmlEscape(title)}</title>`,
          '<style>body { font-family: "Charis SIL", "Doulos SIL", "Gentium Plus", serif; } table { margin: 0 0 1.2em; }</style>',
          "</head><body>",
          `<h1>${htmlEscape(title)}</h1>`,
          ...examples.map(asHtml),
          "</body></html>",
          "",
        ].join("\n"),
      };
    case "text":
      return { extension: "txt", contents: `${title}\n\n${examples.map(asText).join("\n\n")}\n` };
  }
}

/** A segment's place on its tier, as Copy example numbers it: timed segments
 * by start, untimed after them in their own order; 1 for the first. */
export function numbering<T extends { id: string; startMs: number | null }>(segments: readonly T[]): Map<string, number> {
  const order = segments.map((s, i) => ({ s, i })).sort((a, b) => (a.s.startMs ?? Number.MAX_SAFE_INTEGER) - (b.s.startMs ?? Number.MAX_SAFE_INTEGER) || a.i - b.i);
  return new Map(order.map(({ s }, n) => [s.id, n + 1]));
}

/** Whether a sentence is worth an example: at least one word has morphs. */
export const glossed = (ex: Example): boolean => ex.words.some((w) => w.morphs.length > 0);
