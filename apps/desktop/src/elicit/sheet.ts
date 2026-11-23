// A word list on paper: the printed elicitation form field linguists take
// into a session where a laptop is in the way (a village porch, a moving
// car, a speaker who would rather not be typed at). One HTML file to print
// from a browser or open in Word: a header for the session's particulars,
// then a row per concept with its number, gloss and Concepticon gloss, a
// wide empty cell for the word as heard and one for notes. Answers already
// in the project can be printed in grey, to check them, or left out to
// elicit afresh. Pure: the Elicit panel saves the file.

export type SheetConcept = {
  number: number;
  gloss: string;
  concepticonGloss: string;
  skipped: boolean;
  answers: { lemma: string; gloss: string }[];
};

export type SheetOptions = {
  list: { name: string; citation: string };
  language: string;
  concepts: SheetConcept[];
  /** "all" prints every concept; "todo" only those with no answer and not set aside. */
  which: "all" | "todo";
  /** Print answers already in the project, in grey, for checking. */
  withAnswers: boolean;
  generator: string;
};

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** The concepts the sheet prints, in list order. */
export function sheetRows(o: Pick<SheetOptions, "concepts" | "which">): SheetConcept[] {
  const rows = o.which === "todo" ? o.concepts.filter((c) => !c.skipped && c.answers.length === 0) : o.concepts;
  return [...rows].sort((a, b) => a.number - b.number);
}

/** The sheet as one HTML document, A4 or Letter, with a header to fill in by hand. */
export function elicitationSheet(o: SheetOptions): string {
  const rows = sheetRows(o);
  const title = `${o.list.name}${o.language.trim() ? ` — ${o.language.trim()}` : ""}`;
  const field = (label: string) => `<div class="field"><span>${esc(label)}</span><span class="line"></span></div>`;
  const answerCell = (c: SheetConcept) => {
    if (!o.withAnswers) return "";
    if (c.skipped) return `<span class="had">set aside</span>`;
    return c.answers.map((a) => `<span class="had">${esc(a.lemma)}${a.gloss && a.gloss !== c.gloss ? ` ‘${esc(a.gloss)}’` : ""}</span>`).join("<br>");
  };
  const body = rows
    .map((c) => {
      const cg = c.concepticonGloss && c.concepticonGloss.toUpperCase() !== c.gloss.toUpperCase() ? `<div class="cg">${esc(c.concepticonGloss)}</div>` : "";
      return `<tr><td class="n">${c.number}</td><td class="g">${esc(c.gloss)}${cg}</td><td class="w">${answerCell(c)}</td><td class="x"></td></tr>`;
    })
    .join("\n");
  const style = `@page{margin:15mm}body{font-family:"Charis SIL","Gentium Plus","Doulos SIL",serif;font-size:11pt;margin:0 auto;max-width:180mm}
h1{font-size:15pt;margin:0 0 2pt}.cite{color:#555;font-size:9pt;margin:0 0 8pt}
.head{display:grid;grid-template-columns:1fr 1fr;gap:4pt 16pt;margin-bottom:10pt}.field{display:flex;gap:6pt;align-items:flex-end}.field span:first-child{white-space:nowrap}.line{flex:1;border-bottom:1px solid #000;height:14pt}
table{border-collapse:collapse;width:100%}th{text-align:left;font-size:9pt;border-bottom:1.5px solid #000;padding:2pt 4pt}
td{border-bottom:1px solid #999;padding:4pt;vertical-align:top;height:20pt}tr{page-break-inside:avoid}thead{display:table-header-group}
.n{width:8mm;color:#555;text-align:right}.g{width:42mm}.cg{color:#666;font-size:8.5pt;font-variant:small-caps}.w{width:auto}.x{width:40mm}.had{color:#888}
.foot{color:#777;font-size:8pt;margin-top:8pt}`;
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>${esc(title)}</title>
<style>${style}</style>
</head>
<body>
<h1>${esc(title)}</h1>
${o.list.citation ? `<p class="cite">${esc(o.list.citation)}</p>` : ""}
<div class="head">${field("Speaker")}${field("Date")}${field("Place")}${field("Recording")}${field("Elicited by")}${field("Contact language")}</div>
<table>
<thead><tr><th>#</th><th>Concept</th><th>Word as heard</th><th>Notes</th></tr></thead>
<tbody>
${body}
</tbody>
</table>
<p class="foot">${rows.length} ${rows.length === 1 ? "concept" : "concepts"}. Printed by ${esc(o.generator)}; type the answers into the Elicit panel's list after the session.</p>
</body>
</html>
`;
}
