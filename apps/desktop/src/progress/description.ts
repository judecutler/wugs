// A description of the corpus for a data paper, an archive deposit or a
// grant report: what the collection holds, how far it is annotated, who
// speaks in it and how it may be shared, in one HTML file with places left
// to write. Built from the Progress panel's own figures. Pure: no DOM.

import { composition } from "./composition.ts";
import { duration, percent, totals, type Progress } from "./logic.ts";
import type { SpeakerRow } from "./speakers.ts";

export type DescriptionInput = {
  progress: Progress;
  speakers: readonly SpeakerRow[];
  language: { name: string; glottocode: string; iso: string };
  /** Texts whose sharing level is not open. */
  restricted: number;
  /** Texts no consent record covers, when known. */
  uncovered: number | null;
  date: string;
  generator: string;
};

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const n = (x: number) => x.toLocaleString("en");
const todo = (what: string) => `<span class="todo">[${esc(what)}]</span>`;

/** The details most texts give, the ones a description sums the corpus by. */
export function describedKeys(p: Progress): string[] {
  const want = ["genre", "speakers", "speaker", "place", "date", "dialect", "variety"];
  const have = new Set(p.texts.flatMap((t) => (t.meta ?? []).map(([k]) => k)));
  return want.filter((k) => have.has(k));
}

/** The speakers' table: each track with its sex and age from the People panel when any is set (never their names, which the
 * description may be shared without), and a place to say the rest. */
function speakerTable(rows: readonly SpeakerRow[]): string {
  const who = rows.some((s) => s.sex !== "" || s.age !== "");
  const head = ["Track", ...(who ? ["Sex", "Age"] : []), "Texts", "Utterances", "Words"].map((c) => `<th>${c}</th>`).join("");
  const body = rows.map((s) => `<tr>${[esc(s.name), ...(who ? [esc(s.sex), esc(s.age)] : []), n(s.texts), n(s.utterances), n(s.words)].map((c) => `<td>${c}</td>`).join("")}</tr>`).join("");
  const rest = who ? "Who the speakers are: roles and how they were chosen, as their consent allows" : "Who the speakers are: ages, roles, how they were chosen, as their consent allows";
  return `<table><thead><tr>${head}</tr></thead><tbody>${body}</tbody></table><p>${todo(rest)}</p>`;
}

export function corpusDescription(o: DescriptionInput): string {
  const p = o.progress;
  const t = totals(p);
  const lang = o.language.name.trim() || "the language";
  const codes = [o.language.iso && `ISO 639-3 ${o.language.iso}`, o.language.glottocode && `Glottocode ${o.language.glottocode}`].filter(Boolean).join(", ");
  const title = `${o.language.name.trim() || "Corpus"}: description of the collection`;
  const row = (label: string, value: string) => `<tr><th scope="row">${esc(label)}</th><td>${value}</td></tr>`;
  const contents = [
    row("Texts", n(p.texts.length)),
    row("Recorded", esc(duration(p.recordedMs)) + (p.recordingsMissing > 0 ? ` (${p.recordingsMissing} not on the computer that wrote this)` : "")),
    row("Transcribed", `${n(t.transcribed)} of ${n(t.sentences)} sentences (${percent(t.transcribed, t.sentences)}), ${esc(duration(t.transcribedMs))} of timed speech`),
    row("Words", n(t.words)),
    row("Glossed", `${n(t.glossedWords)} words (${percent(t.glossedWords, t.words)})`),
    row("Translated", `${n(t.translated)} sentences (${percent(t.translated, t.transcribed)})`),
    row("Lexicon", `${n(p.entries)} entries, ${n(p.senses)} senses`),
    row("Speakers", n(o.speakers.length)),
  ].join("");
  const byKey = describedKeys(p)
    .map((k) => {
      const rows = composition(p.texts, k).filter((r) => r.texts > 0);
      if (rows.length === 0) return "";
      const body = rows.map((r) => `<tr><td>${esc(r.value)}</td><td>${n(r.texts)}</td><td>${n(r.words)}</td></tr>`).join("");
      return `<h3>By ${esc(k.replace(/_/g, " "))}</h3><table><thead><tr><th>${esc(k.replace(/_/g, " "))}</th><th>Texts</th><th>Words</th></tr></thead><tbody>${body}</tbody></table>`;
    })
    .join("");
  const speakers =
    o.speakers.length === 0
      ? `<p>${todo("Who speaks in the recordings: number, ages, roles, how they were chosen")}</p>`
      : speakerTable(o.speakers);
  const access = [
    o.restricted === 0 ? "Every text is open to share." : `${n(o.restricted)} of ${n(p.texts.length)} texts have a sharing level other than open and are left out of public exports.`,
    o.uncovered === null ? "" : o.uncovered === 0 ? "A consent record covers every text." : `${n(o.uncovered)} ${o.uncovered === 1 ? "text has" : "texts have"} no consent record yet.`,
  ]
    .filter(Boolean)
    .join(" ");
  const style = `body{font-family:"Charis SIL","Gentium Plus","Noto Serif",Georgia,serif;font-size:11.5pt;line-height:1.45;max-width:46em;margin:2em auto;padding:0 1em;color:#1b1a17}
h1{font-size:1.6em;margin-bottom:.1em}h2{font-size:1.25em;margin-top:1.6em;border-bottom:1px solid #ddd}h3{font-size:1.05em}
.meta{color:#555}table{border-collapse:collapse;margin:.5em 0}th,td{border-bottom:1px solid #ddd;padding:3px 10px 3px 0;text-align:left;vertical-align:top}
td:not(:first-child){font-variant-numeric:tabular-nums}.todo{background:#fff3c4;color:#6b5200}`;
  return `<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"><title>${esc(title)}</title><meta name="generator" content="${esc(o.generator)}"><style>${style}</style></head>
<body>
<h1>${esc(title)}</h1>
<p class="meta">Figures as of ${esc(o.date)}. Places in yellow are for you to write.</p>
<h2>1. The language</h2>
<p>${esc(lang)}${codes ? ` (${esc(codes)})` : ""}. ${todo("Where it is spoken, by how many people, its family and its situation")}</p>
<h2>2. The collection</h2>
<p>${todo("Who made the recordings, when, where and how; the equipment; the aims of the project")}</p>
<table>${contents}</table>
${byKey}
<h2>3. Speakers</h2>
${speakers}
<h2>4. Annotation</h2>
<p>Sentences are transcribed${t.transcribed > 0 ? "" : " (none yet)"}, ${percent(t.glossedWords, t.words)} of the words are glossed morpheme by morpheme following the Leipzig Glossing Rules, and ${percent(t.translated, t.transcribed)} of the transcribed sentences have a free translation. ${todo("The transcription conventions and orthography; who transcribed, glossed and translated")}</p>
<h2>5. Access and consent</h2>
<p>${esc(access)} ${todo("The archive, the licence, and how to ask for restricted material")}</p>
<h2>6. How to cite</h2>
<p>${todo("Authors")} (${esc(o.date.slice(0, 4))}). <i>${esc(o.language.name.trim() || "Title")} collection</i>. ${todo("Archive, identifier or DOI")}.</p>
<p class="meta">Made with ${esc(o.generator)}.</p>
</body></html>
`;
}
