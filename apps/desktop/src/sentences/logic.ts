// Sentence elicitation: a list of prompts worked through one by one, each
// answer a sentence with the prompt as its translation
// (src-tauri/src/sentences.rs). Pure, npm-tested.

export type PromptList = { id: string; name: string; total: number; answered: number; textId: string | null };
export type PromptAnswer = { segmentId: string; tierId: string; content: string };
export type PromptRow = { id: string; number: number; prompt: string; answers: PromptAnswer[] };
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
  "Load a list of prompts: a .txt with one sentence a line, or a table with a prompt column (and an id column if you like). Each answer becomes a sentence of the list's own text, with the prompt as its translation, ready to gloss.";
