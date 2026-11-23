// Find and replace across texts (src-tauri/src/replace.rs): what the
// preview shows and what the toast says, pure so it can be tested.

export type Track = "transcription" | "translation" | "headwords";

export type Find = {
  find: string;
  with: string;
  track: Track;
  wholeWord: boolean;
  matchCase: boolean;
  /** One text, or every text. */
  text: string | null;
};

export type Change = {
  textId: string;
  textTitle: string;
  tierId: string;
  segmentId: string;
  before: string;
  after: string;
  count: number;
  glossed: boolean;
  /** Kept text, then what one place held and will hold; only in a preview. */
  parts?: Part[];
  /** A headword's field (`lemma`, `citation`, `form`); absent for a sentence. */
  field?: string | null;
};

export type Part = { keep: string; removed: string; added: string };

export type Preview = { changes: Change[]; total: number };
export type Applied = { done: Change[]; skipped: number };

export const TRACKS: { track: Track; label: string }[] = [
  { track: "transcription", label: "Transcriptions" },
  { track: "translation", label: "Translations" },
  { track: "headwords", label: "Headwords" },
];

/** Whether these changes are headwords rather than sentences. */
const ofHeadwords = (changes: readonly Change[]): boolean => changes.some((c) => c.field != null);
const things = (n: number, headwords: boolean) => (headwords ? (n === 1 ? "1 headword" : `${n} headwords`) : sentences(n));

/** The undo of `done`: the same sentences with before and after swapped. */
export const undoOf = (done: Change[]): Change[] => done.map((c) => ({ ...c, before: c.after, after: c.before }));

const sentences = (n: number) => (n === 1 ? "1 sentence" : `${n} sentences`);

/** The line over the preview list. */
export function previewNote(p: Preview, find: string): string {
  if (find.trim() === "") return "Type what to find. It is matched exactly as typed, with no pattern syntax.";
  if (p.total === 0) return "Nothing to change.";
  const places = p.changes.reduce((n, c) => n + c.count, 0);
  const shown = p.total > p.changes.length ? ` The first ${p.changes.length} are listed; replace them, then preview again for the rest.` : "";
  const placesWord = places === 1 ? "1 place" : `${places} places`;
  return `${placesWord} in ${things(p.total, ofHeadwords(p.changes))}.${shown}`;
}

/** The replace button's label for the ticked sentences. */
export const replaceLabel = (ticked: number, headwords = false): string => (ticked === 0 ? "Replace" : `Replace in ${things(ticked, headwords)}`);

/** What the toast says after a replace (`undo`: this was the undo). */
export function appliedLines(a: Applied, undo = false): string[] {
  const hw = ofHeadwords(a.done);
  const lines = [undo ? `Put back ${things(a.done.length, hw)}.` : `Replaced in ${things(a.done.length, hw)}.`];
  if (a.skipped > 0) lines.push(`${things(a.skipped, hw)} changed since the preview and ${a.skipped === 1 ? "was" : "were"} left alone.`);
  if (hw && !undo) lines.push("Sentences in the texts keep the old spelling: replace in Transcriptions as well to change them there.");
  const glossed = a.done.filter((c) => c.glossed).length;
  if (glossed > 0 && !undo)
    lines.push(
      glossed === 1
        ? "1 glossed sentence now reads differently from its words: Retokenize it in Gloss to bring them in line."
        : `${glossed} glossed sentences now read differently from their words: Retokenize them in Gloss to bring the words in line.`,
    );
  return lines;
}
