// Sentence elicitation: a list of prompts worked through one by one, each
// answer a sentence with the prompt as its translation
// (src-tauri/src/sentences.rs). Pure, npm-tested.

export type PromptList = { id: string; name: string; total: number; answered: number; textId: string | null };
export type PromptAnswer = { segmentId: string; tierId: string; content: string };
export type PromptRow = { id: string; number: number; prompt: string; /** The word a frame asks for; never saved with the answer. */ hint?: string | null; answers: PromptAnswer[] };
export type PromptView = { list: PromptList; prompts: PromptRow[] };

/** "12 of 40 prompts answered." */
export function progressLine(l: PromptList): string {
  if (l.total === 0) return "This list has no prompts.";
  if (l.answered === l.total) return `Every one of the ${l.total} prompts has an answer.`;
  return `${l.answered} of ${l.total} ${l.total === 1 ? "prompt" : "prompts"} answered.`;
}

/** The prompts listed: every one, or only those still without an answer. */
export const shownPrompts = (rows: readonly PromptRow[], toDoOnly: boolean): PromptRow[] => (toDoOnly ? rows.filter((r) => r.answers.length === 0) : [...rows]);

/** The next prompt without an answer after `id` (wrapping round), or null when none is left. */
export function nextToDo(rows: readonly PromptRow[], id: string | null): PromptRow | null {
  const at = id === null ? -1 : rows.findIndex((r) => r.id === id);
  for (let k = 1; k <= rows.length; k++) {
    const r = rows[(at + k + rows.length) % rows.length]!;
    if (r.answers.length === 0 && r.id !== id) return r;
  }
  return null;
}

/** The empty state's words when the project has no prompt list. */
export const NO_LISTS =
  "Load a list of prompts: a .txt with one sentence a line, or a table with a prompt column (and an id column if you like). Or press From frames… to put each lexicon entry in sentence frames of your own. Each answer becomes a sentence of the list's own text, with the prompt as its translation, ready to gloss.";

/** The blank a frame marks for the word: three underscores or more. */
const BLANK = /_{3,}/u;

export type FrameEntry = { id: string; lemma: string; homograph: number | null; posOfSpeech: string; glosses: string[] };
export type FramePrompt = { id: string; prompt: string; hint: string };
export type Framed = { prompts: FramePrompt[]; frames: number; entries: number; noGloss: number; notFrames: string[] };

/** A gloss to put in a frame: the first of a sense's glosses, up to `;` or `,`. */
export const frameGloss = (glosses: readonly string[]): string => (glosses.find((g) => g.trim() !== "") ?? "").split(/[;,]/u)[0]!.trim();

/** A headword with its homograph number written after it (tela2). */
const headwordOf = (e: FrameEntry): string => (e.homograph === null ? e.lemma : `${e.lemma}${e.homograph}`);

/** Prompts for frame elicitation: each entry (of one category, or every one)
 * in each frame, the frame's blank (___) filled with the entry's gloss, the
 * headword kept as the hint. Entry by entry, so a word's frames come one
 * after another, as tone and noun-class frames are asked. Lines with no
 * blank are not frames; entries with no gloss are left out. */
export function framePrompts(frames: string, entries: readonly FrameEntry[], category: string | null): Framed {
  const lines = frames
    .split(/\r?\n/u)
    .map((l) => l.trim())
    .filter((l) => l !== "");
  const ok = lines.filter((l) => BLANK.test(l));
  const notFrames = lines.filter((l) => !BLANK.test(l));
  const picked = entries.filter((e) => category === null || e.posOfSpeech === category);
  const prompts: FramePrompt[] = [];
  let noGloss = 0;
  for (const e of picked) {
    const gloss = frameGloss(e.glosses);
    if (gloss === "") {
      noGloss += 1;
      continue;
    }
    ok.forEach((frame, i) => prompts.push({ id: `${e.id}@${i + 1}`, prompt: frame.replace(BLANK, gloss), hint: headwordOf(e) }));
  }
  return { prompts, frames: ok.length, entries: picked.length - noGloss, noGloss, notFrames };
}

/** What making a list from frames did, in words. */
export function framedLine(f: Framed): string {
  if (f.frames === 0) return "Write at least one frame with ___ where the word goes, such as «I see the ___».";
  const parts = [`${f.prompts.length} ${f.prompts.length === 1 ? "prompt" : "prompts"}: ${f.entries} ${f.entries === 1 ? "word" : "words"} in ${f.frames} ${f.frames === 1 ? "frame" : "frames"}.`];
  if (f.noGloss > 0) parts.push(`${f.noGloss} ${f.noGloss === 1 ? "entry has" : "entries have"} no gloss to put in a frame and ${f.noGloss === 1 ? "was" : "were"} left out.`);
  if (f.notFrames.length > 0) parts.push(`${f.notFrames.length === 1 ? "1 line has" : `${f.notFrames.length} lines have`} no ___ and ${f.notFrames.length === 1 ? "is" : "are"} not a frame.`);
  return parts.join(" ");
}
