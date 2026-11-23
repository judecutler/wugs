// The Notes panel's rules: which notes show, the words above them, and the
// list copied or printed to take to the next session. Pure, npm-tested.

export type SentenceNote = { noteId: string; body: string; textId: string; textTitle: string; tierId: string; segmentId: string; content: string };
export type EntryNote = { entryId: string; headword: string; homograph: number | null; sense: number | null; gloss: string; body: string };
export type Notes = { sentences: SentenceNote[]; entries: EntryNote[] };

export type Which = "all" | "sentences" | "entries";
export const WHICH: [Which, string][] = [
  ["all", "All"],
  ["sentences", "Sentences"],
  ["entries", "Lexicon"],
];

export type Filter = { which: Which; query: string; questions: boolean };

const fold = (s: string): string => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

/** A question: the note asks something (a "?" anywhere). */
export const isQuestion = (body: string): boolean => /[?？¿]/.test(body);

function keep(f: Filter, body: string, ...also: string[]): boolean {
  if (f.questions && !isQuestion(body)) return false;
  const q = fold(f.query.trim());
  return q === "" || [body, ...also].some((s) => fold(s).includes(q));
}

/** The notes that show under a filter: the query is looked for in the note,
 * its sentence or headword and gloss, ignoring case and accents. */
export function shown(n: Notes, f: Filter): Notes {
  return {
    sentences: f.which === "entries" ? [] : n.sentences.filter((s) => keep(f, s.body, s.content, s.textTitle)),
    entries: f.which === "sentences" ? [] : n.entries.filter((e) => keep(f, e.body, e.headword, e.gloss)),
  };
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** The line above the list. */
export function notesLine(all: Notes, now: Notes): string {
  const total = all.sentences.length + all.entries.length;
  if (total === 0) return "No notes yet. Add them under a sentence in the Gloss panel, or on an entry or sense in the Lexicon.";
  const showing = now.sentences.length + now.entries.length;
  const questions = [...all.sentences, ...all.entries].filter((n) => isQuestion(n.body)).length;
  const parts = `${plural(all.sentences.length, "note on a sentence", "notes on sentences")} and ${plural(all.entries.length, "note in the lexicon", "notes in the lexicon")}, ${questions} of them ${questions === 1 ? "a question" : "questions"}`;
  return showing === total ? `${parts}.` : `${parts}; ${showing} shown.`;
}

const subscripts = "₀₁₂₃₄₅₆₇₈₉";
/** A headword with its homograph number as a subscript, as the dictionary prints it. */
export const headwordOf = (e: Pick<EntryNote, "headword" | "homograph">): string =>
  e.homograph === null ? e.headword : e.headword + String(e.homograph).replace(/\d/g, (d) => subscripts[Number(d)]!);

/** Where an entry note is: the entry itself, or its sense with the gloss. */
export function entryPlace(e: EntryNote): string {
  const gloss = e.gloss.trim() === "" ? "" : ` ‘${e.gloss.trim()}’`;
  return e.sense === null ? `${headwordOf(e)}${gloss}` : `${headwordOf(e)}, sense ${e.sense}${gloss}`;
}

/** The list as plain text, to paste into a notebook or an email. */
export function notesText(n: Notes): string {
  const out: string[] = [];
  let title: string | null = null;
  for (const s of n.sentences) {
    if (s.textTitle !== title) {
      if (out.length > 0) out.push("");
      out.push(s.textTitle || "Untitled text");
      title = s.textTitle;
    }
    out.push(`- ${s.content}`, `  ${s.body}`);
  }
  if (n.entries.length > 0) {
    if (out.length > 0) out.push("");
    out.push("Lexicon");
    for (const e of n.entries) out.push(`- ${entryPlace(e)}`, `  ${e.body}`);
  }
  return out.join("\n") + (out.length > 0 ? "\n" : "");
}

const esc = (s: string): string => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** A printable page of the notes, with a box beside each to tick in the session. */
export function notesSheet(n: Notes, o: { language: string; date: string; generator: string }): string {
  const title = `${o.language.trim() || "Project"}: notes to go through`;
  const item = (where: string, body: string) =>
    `<li><span class="box"></span><div><div class="where">${esc(where)}</div><div class="body">${esc(body)}</div><div class="answer"></div></div></li>`;
  const groups: string[] = [];
  let title0: string | null = null;
  let items: string[] = [];
  const flush = () => {
    if (title0 !== null) groups.push(`<h2>${esc(title0 || "Untitled text")}</h2><ol>${items.join("")}</ol>`);
    items = [];
  };
  for (const s of n.sentences) {
    if (s.textTitle !== title0) {
      flush();
      title0 = s.textTitle;
    }
    items.push(item(s.content, s.body));
  }
  flush();
  if (n.entries.length > 0) groups.push(`<h2>Lexicon</h2><ol>${n.entries.map((e) => item(entryPlace(e), e.body)).join("")}</ol>`);
  const style = `@page{margin:15mm}body{font-family:"Charis SIL","Gentium Plus","Doulos SIL",serif;font-size:11pt;margin:0 auto;max-width:180mm}
h1{font-size:16pt;margin:0 0 2pt}h2{font-size:12.5pt;margin:14pt 0 4pt;break-after:avoid}.meta{color:#555;margin:0 0 10pt}
ol{list-style:none;padding:0;margin:0}li{display:flex;gap:8pt;padding:5pt 0;border-bottom:1px solid #ccc;break-inside:avoid}
.box{flex:none;width:10pt;height:10pt;border:1px solid #333;margin-top:2pt}.where{font-weight:bold}.body{margin-top:1pt}
.answer{height:22pt}`;
  return `<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"><title>${esc(title)}</title><meta name="generator" content="${esc(o.generator)}"><style>${style}</style></head>
<body><h1>${esc(title)}</h1><p class="meta">${esc(o.date)}. Speaker: ______________________</p>${groups.join("\n")}</body></html>
`;
}
