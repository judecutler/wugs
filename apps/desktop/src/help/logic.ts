// Pure rules for the Help sheet (roadmap 67): what is sent, what is shown.

/** One turn of the conversation as the service reads it. */
export interface Turn {
  role: "user" | "assistant";
  content: string;
}

/** A question and what became of it, as the sheet shows it. */
export interface Exchange {
  question: string;
  answer: string | null;
  sources: string[];
  problem: string | null;
}

export const MAX_QUESTION_CHARS = 1000;
/** How many earlier turns go with a question (the service keeps the last 6). */
export const HISTORY_TURNS = 6;
/** Remembered per machine; off until the person ticks it. */
export const SHARE_KEY = "wugs.help.share";

/** Why the question cannot be asked yet, or null. */
export function askBlocker(text: string): string | null {
  const t = text.trim();
  if (t === "") return "Type a question first.";
  if (t.length > MAX_QUESTION_CHARS) return `Questions can be up to ${MAX_QUESTION_CHARS} characters; this one has ${t.length}.`;
  return null;
}

/** The answered exchanges before this question, as turns, newest last.
 * Unanswered ones are left out so a failed try never confuses the next. */
export function historyOf(exchanges: readonly Exchange[]): Turn[] {
  const turns: Turn[] = [];
  for (const e of exchanges) {
    if (e.answer === null) continue;
    turns.push({ role: "user", content: e.question }, { role: "assistant", content: e.answer });
  }
  return turns.slice(-HISTORY_TURNS);
}

/** "From the guide: A, B" under an answer, or null when it used none. */
export function sourcesLine(sources: readonly string[]): string | null {
  return sources.length === 0 ? null : `From the user guide: ${sources.join("; ")}`;
}

export function shareOn(storage: Pick<Storage, "getItem">): boolean {
  return storage.getItem(SHARE_KEY) === "on";
}
