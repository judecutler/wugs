// A lexicon checklist to print for a session with a speaker: each entry
// listed (headword, category, glosses) with a box to tick and room for a
// correction, so the dictionary is checked line by line on paper. Pure.

export type ChecklistEntry = { lemma: string; homograph: number | null; posOfSpeech: string; glosses: string[] };

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** The checklist as one HTML file; `which` names the entries listed (Every entry, Needs a gloss, noun…). */
export function checklistSheet(entries: readonly ChecklistEntry[], language: string, which: string): string {
  const title = `${language.trim() === "" ? "Lexicon" : `${language.trim()} lexicon`}: entries to check`;
  const field = (label: string) => `<div class="field"><span>${esc(label)}</span><span class="line"></span></div>`;
  const rows = entries
    .map(
      (e, i) =>
        `<tr><td class="n">${i + 1}</td><td class="hw">${esc(e.lemma)}${e.homograph === null ? "" : `<sub>${e.homograph}</sub>`}</td><td class="pos">${esc(e.posOfSpeech)}</td><td>${esc(e.glosses.filter((g) => g.trim() !== "").join("; "))}</td><td class="ok">☐</td><td></td></tr>`,
    )
    .join("\n");
  const style = `@page{margin:14mm}body{font-family:"Charis SIL","Gentium Plus","Doulos SIL",serif;font-size:10.5pt;margin:0 auto;max-width:185mm}
h1{font-size:15pt;margin:0 0 4pt}.which{color:#555;margin:0 0 10pt}.head{display:grid;grid-template-columns:1fr 1fr 1fr;gap:4pt 14pt;margin-bottom:10pt}.field{display:flex;gap:6pt;align-items:flex-end}.field span:first-child{white-space:nowrap}.line{flex:1;border-bottom:1px solid #000;height:14pt}
table{border-collapse:collapse;width:100%}th,td{border:1px solid #888;padding:3pt 5pt;text-align:left;vertical-align:top}th{font-size:9pt;background:#f2f2f2}thead{display:table-header-group}tr{page-break-inside:avoid}
td.n{color:#777;width:8mm}td.hw{font-weight:bold}td.pos{font-style:italic;width:18mm}td.ok{text-align:center;width:9mm}td:last-child{width:55mm}.foot{color:#777;font-size:8pt;margin-top:8pt}`;
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>${esc(title)}</title>
<style>${style}</style>
</head>
<body>
<h1>${esc(title)}</h1>
<p class="which">${esc(which)}: ${entries.length} ${entries.length === 1 ? "entry" : "entries"}</p>
<div class="head">${field("Checked with")}${field("Date")}${field("Place")}</div>
<table>
<thead><tr><th>No.</th><th>Headword</th><th>Category</th><th>Meaning</th><th>OK</th><th>Correction, notes</th></tr></thead>
<tbody>
${rows}
</tbody>
</table>
<p class="foot">Printed from wugs. Tick what the speaker confirms; write corrections beside the rest, then make them in the Lexicon.</p>
</body>
</html>
`;
}
