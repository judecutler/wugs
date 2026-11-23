// Progress: how far the documentation has come (src-tauri/src/progress.rs).
// Pure: the words, percentages and the table a grant report or deposit form
// asks for. No DOM.

export type TextProgress = {
  id: string;
  title: string;
  recordingMs: number | null;
  sentences: number;
  transcribed: number;
  transcribedMs: number;
  words: number;
  glossedWords: number;
  /** The morphs of the glossed words. */
  morphs: number;
  translated: number;
};

export type Progress = {
  texts: TextProgress[];
  recordedMs: number;
  recordingsMissing: number;
  entries: number;
  senses: number;
  sensesWithoutGloss: number;
  people: number;
  consentRecords: number;
};

/** A length people read: "2 h 05 min", "12 min 30 s", "40 s", "—" for none. */
export function duration(ms: number | null): string {
  if (ms === null) return "—";
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min ${String(s % 60).padStart(2, "0")} s`;
  return `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, "0")} min`;
}

/** A share as a whole percent, "—" when there is nothing to share out. */
export function percent(part: number, whole: number): string {
  if (whole <= 0) return "—";
  const p = (part * 100) / whole;
  return p > 0 && p < 1 ? "<1%" : `${Math.round(p)}%`;
}

/** The texts summed, as one row. */
export function totals(p: Progress): TextProgress {
  const sum = (f: (t: TextProgress) => number) => p.texts.reduce((n, t) => n + f(t), 0);
  return {
    id: "",
    title: "All texts",
    recordingMs: p.recordedMs,
    sentences: sum((t) => t.sentences),
    transcribed: sum((t) => t.transcribed),
    transcribedMs: sum((t) => t.transcribedMs),
    words: sum((t) => t.words),
    glossedWords: sum((t) => t.glossedWords),
    morphs: sum((t) => t.morphs),
    translated: sum((t) => t.translated),
  };
}

/** The headline figures, a line each. */
export function summaryLines(p: Progress): string[] {
  const t = totals(p);
  const n = (x: number) => x.toLocaleString("en");
  const lines = [
    `${n(p.texts.length)} ${p.texts.length === 1 ? "text" : "texts"}, ${duration(p.recordedMs)} recorded${p.recordingsMissing > 0 ? ` (${p.recordingsMissing} ${p.recordingsMissing === 1 ? "recording is" : "recordings are"} not on this computer and not counted)` : ""}.`,
    `${n(t.transcribed)} of ${n(t.sentences)} sentences transcribed (${percent(t.transcribed, t.sentences)}), covering ${duration(t.transcribedMs)} of timed speech.`,
    `${n(t.words)} words; ${n(t.glossedWords)} glossed (${percent(t.glossedWords, t.words)}); ${n(t.translated)} sentences translated (${percent(t.translated, t.transcribed)}).`,
    ...shapeLine(t),
    `Lexicon: ${n(p.entries)} ${p.entries === 1 ? "entry" : "entries"}, ${n(p.senses)} ${p.senses === 1 ? "sense" : "senses"}${p.sensesWithoutGloss > 0 ? `, ${n(p.sensesWithoutGloss)} still without a gloss` : ""}.`,
    `People: ${n(p.people)}, with ${n(p.consentRecords)} consent ${p.consentRecords === 1 ? "record" : "records"}.`,
  ];
  return lines;
}

/** Words per transcribed sentence and morphs per glossed word (Greenberg's
 * index of synthesis), to one decimal; "—" when there is nothing to divide. */
export function ratio(part: number, whole: number): string {
  return whole <= 0 ? "—" : (part / whole).toFixed(1);
}

/** How long the sentences and words are, once there are any. */
function shapeLine(t: TextProgress): string[] {
  if (t.transcribed === 0) return [];
  const words = `Sentences average ${ratio(t.words, t.transcribed)} words`;
  return [t.glossedWords === 0 ? `${words}.` : `${words}; glossed words average ${ratio(t.morphs, t.glossedWords)} morphs (the index of synthesis).`];
}

export const COLUMNS = ["Text", "Recording", "Sentences", "Transcribed", "Speech transcribed", "Words", "Glossed", "Morphs per word", "Translated"] as const;

/** One table row's cells, in `COLUMNS` order. */
export function rowCells(t: TextProgress): string[] {
  const n = (x: number) => x.toLocaleString("en");
  return [
    t.title || "untitled",
    duration(t.recordingMs),
    n(t.sentences),
    `${n(t.transcribed)} (${percent(t.transcribed, t.sentences)})`,
    t.transcribedMs > 0 ? duration(t.transcribedMs) : "—",
    n(t.words),
    `${n(t.glossedWords)} (${percent(t.glossedWords, t.words)})`,
    ratio(t.morphs, t.glossedWords),
    `${n(t.translated)} (${percent(t.translated, t.transcribed)})`,
  ];
}

/** Tab-separated, every text then the totals, numbers bare for a spreadsheet. */
export function progressTsv(p: Progress): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  const head = ["text", "recording (min)", "sentences", "transcribed", "speech transcribed (min)", "words", "glossed words", "morphs", "translated"].join("\t");
  const min = (ms: number | null) => (ms === null ? "" : (ms / 60000).toFixed(1));
  const row = (t: TextProgress) => [clean(t.title), min(t.recordingMs), t.sentences, t.transcribed, min(t.transcribedMs), t.words, t.glossedWords, t.morphs, t.translated].join("\t");
  return [head, ...p.texts.map(row), row(totals(p))].join("\n");
}
