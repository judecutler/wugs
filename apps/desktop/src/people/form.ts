// A consent form to print: the terms the People panel records, each in plain
// words with Yes and No boxes, so what a person says on paper is what gets
// typed into their record afterwards. A starting point for the researcher's
// own approved form, never a replacement for it.
import { termLabel } from "./logic.ts";

/** The terms the People panel records, in its order. */
export const FORM_TERMS = ["archive-ok", "web-ok", "community-only", "AI-ok"] as const;

const EXPLAIN: Record<string, string> = {
  "archive-ok": "The recordings, transcriptions and notes are kept in a language archive, so they last for the community and for research.",
  "web-ok": "Recordings, transcriptions or a dictionary made from them may be put on a website that anyone can see.",
  "community-only": "Only members of the community and the people working on this project may see or hear them.",
  "AI-ok": "Recordings and their transcriptions may be used to build or test computer tools that recognise speech.",
};

/** What a term means, in words a participant can follow. */
export const termExplained = (term: string): string => EXPLAIN[term] ?? "";

export type FormInput = {
  /** The project's language, or empty. */
  language: string;
  /** The person the form is printed for, or empty for a blank form. */
  name: string;
  roles: readonly string[];
};

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** A printable consent form as one HTML file. */
export function consentForm(input: FormInput): string {
  const language = input.language.trim();
  const name = input.name.trim();
  const title = language === "" ? "Consent to record and share" : `Consent to record and share: ${language}`;
  const line = (label: string, value = "") =>
    `<div class="field"><span>${esc(label)}</span><span class="line">${value === "" ? "" : esc(value)}</span></div>`;
  const box = (label: string) => `<span class="box">☐ ${esc(label)}</span>`;
  const terms = FORM_TERMS.map(
    (t) => `<tr><td><strong>${esc(termLabel(t))}</strong><br><span class="why">${esc(termExplained(t))}</span></td><td class="yn">${box("Yes")}${box("No")}</td></tr>`,
  ).join("\n");
  const style = `@page{margin:15mm}body{font-family:"Charis SIL","Gentium Plus","Doulos SIL",serif;font-size:11pt;margin:0 auto;max-width:180mm;line-height:1.35}
h1{font-size:16pt;margin:0 0 10pt}h2{font-size:12pt;margin:14pt 0 6pt}.head{display:grid;grid-template-columns:1fr 1fr;gap:6pt 16pt}.field{display:flex;gap:6pt;align-items:flex-end}.field span:first-child{white-space:nowrap}.line{flex:1;border-bottom:1px solid #000;min-height:15pt}
.box{display:inline-block;margin-right:14pt;white-space:nowrap}table{border-collapse:collapse;width:100%}td{border:1px solid #777;padding:5pt 7pt;vertical-align:top}td.yn{width:34mm}.why{font-size:10pt}
.sign{display:grid;grid-template-columns:1fr 1fr;gap:16pt 16pt;margin-top:8pt}.notes .line{display:block;min-height:18pt}.foot{color:#666;font-size:8.5pt;margin-top:14pt}`;
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>${esc(name === "" ? title : `${title} (${name})`)}</title>
<style>${style}</style>
</head>
<body>
<h1>${esc(title)}</h1>
<div class="head">${line("Name", name)}${line("Role", input.roles.join(", "))}${line("Researcher")}${line("Contact")}${line("Date")}${line("Place")}</div>
<h2>What we are doing</h2>
<div class="notes"><span class="line"></span><span class="line"></span><span class="line"></span></div>
<h2>What will be recorded</h2>
<p>${box("Sound")}${box("Video")}${box("Photographs")}${box("Written notes")}</p>
<h2>What may be done with it</h2>
<table>
${terms}
</table>
<h2>How you would like to be named</h2>
<p>${box("By my name")}${box("By another name")}${box("Not named")}</p>
<p>You may say no to any of these, and change your mind later, without giving a reason. Tell the researcher, and what you no longer agree to will stop.</p>
<h2>Agreed</h2>
<div class="sign">${line("Signed or marked")}${line("Date")}${line("Researcher")}${line("Date")}</div>
<p>If this was agreed aloud: ${box("read aloud")}${box("translated")}</p>
<div class="sign">${line("Language used")}${line("Recording of the agreement")}${line("Witness")}${line("Date")}</div>
<p class="foot">Printed from wugs as a starting point. Your ethics board's approved wording comes first; translate it into a language the person reads well. Afterwards, enter the answers in the People panel and attach a scan of this page.</p>
</body>
</html>
`;
}
