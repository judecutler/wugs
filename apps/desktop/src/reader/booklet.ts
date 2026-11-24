// A reader to print: one text, sentence by sentence, in the language with its
// translation beside it (or under it on a narrow page), as literacy
// programmes and community language classes hand out. One HTML file that
// prints to PDF or opens in Word; no glosses, nothing the reader needs to
// know about linguistics. Pure, npm-tested.

/** One sentence; `speaker` names who said it in a transcript with a track per speaker. */
export type ReaderSentence = { number: number; text: string; translation: string | null; speaker?: string };

export type ReaderInput = {
  title: string;
  /** The language's name, if the project has one. */
  language: string;
  /** Details of the text to print under the title (speaker, place, date…). */
  details: [string, string][];
  sentences: ReaderSentence[];
  /** How the pages are laid out: side by side, or each translation under its sentence. */
  layout: "beside" | "under";
  date: string;
};

const esc = (s: string): string => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** The details worth printing for a reader: who told it, where and when. */
export function readerDetails(meta: readonly [string, string][]): [string, string][] {
  const wanted = ["speakers", "speaker", "author", "narrator", "place", "date", "genre"];
  return wanted.flatMap((k) => meta.filter(([key, v]) => key === k && v.trim() !== "")).map(([k, v]) => [k.charAt(0).toUpperCase() + k.slice(1), v]);
}

export function readerHtml(r: ReaderInput): string {
  const translated = r.sentences.some((s) => s.translation !== null && s.translation.trim() !== "");
  const rows = r.sentences.map((s) => {
    const t = s.translation?.trim() ?? "";
    const who = s.speaker === undefined || s.speaker === "" ? "" : `<b class="who">${esc(s.speaker)}:</b> `;
    if (r.layout === "beside" && translated) {
      return `<tr><td class="n">${s.number}</td><td class="tx">${who}${esc(s.text)}</td><td class="tr">${esc(t)}</td></tr>`;
    }
    return `<div class="s"><span class="n">${s.number}</span><div><p class="tx">${who}${esc(s.text)}</p>${t === "" ? "" : `<p class="tr">${esc(t)}</p>`}</div></div>`;
  });
  const body = r.layout === "beside" && translated ? `<table>${rows.join("\n")}</table>` : rows.join("\n");
  const details = r.details.length === 0 ? "" : `<p class="details">${r.details.map(([k, v]) => `${esc(k)}: ${esc(v)}`).join(" · ")}</p>`;
  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>${esc(r.title)}</title>
<style>
@page { margin: 18mm; }
body { font-family: "Charis SIL", "Doulos SIL", "Gentium Plus", Georgia, serif; font-size: 14pt; line-height: 1.5; color: #111; max-width: 46em; margin: 2em auto; padding: 0 1em; }
h1 { font-size: 22pt; margin: 0 0 0.2em; }
.lang { font-size: 11pt; color: #555; margin: 0; letter-spacing: 0.04em; text-transform: uppercase; }
.details { font-size: 11pt; color: #444; margin: 0.4em 0 1.6em; }
table { border-collapse: collapse; width: 100%; }
td { vertical-align: top; padding: 0.35em 0.6em 0.35em 0; }
tr { break-inside: avoid; }
.n { color: #888; font-size: 10pt; width: 2em; padding-top: 0.55em; }
td.tx { width: 50%; font-weight: 600; }
td.tr { width: 50%; color: #333; font-style: italic; }
.s { display: flex; gap: 0.6em; margin: 0 0 0.9em; break-inside: avoid; }
.s p { margin: 0; }
.s .tx { font-weight: 600; }
.who { font-weight: 700; color: #555; margin-right: 0.3em; }
.s .tr { color: #333; font-style: italic; }
footer { margin-top: 2.5em; font-size: 9pt; color: #888; }
</style>
</head>
<body>
${r.language.trim() === "" ? "" : `<p class="lang">${esc(r.language)}</p>`}
<h1>${esc(r.title)}</h1>
${details}
${body}
<footer>Made with wugs on ${esc(r.date)}.</footer>
</body>
</html>
`;
}
