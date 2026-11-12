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

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** One frame of the wug's jump: where its box's top-left is and how big it is. */
export interface HopFrame {
  x: number;
  y: number;
  size: number;
  /** 0 to 1 along the jump. */
  t: number;
}

/** The jump from the top bar's wug into the Help panel's: a parabola whose
 * top is `lift` px above the higher of the two, the size easing from one box
 * to the other. `steps` frames after the first. */
export function hopFrames(from: Box, to: Box, steps = 12, lift = 70): HopFrame[] {
  const top = Math.min(from.y, to.y) - lift;
  // y(t) = a t² + b t + c through (0, from.y), its peak at `top`, and (1, to.y).
  const y0 = from.y;
  const y1 = to.y;
  // Peak height h above the lower end; solve for b with the vertex at top.
  const p = y0 - top;
  const q = y1 - top;
  const b = -2 * (p + Math.sqrt(p * q));
  const a = y1 - y0 - b;
  const frames: HopFrame[] = [];
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    frames.push({
      t,
      x: from.x + (to.x - from.x) * t,
      y: a * t * t + b * t + y0,
      size: from.width + (to.width - from.width) * t,
    });
  }
  return frames;
}

/** How much of an answer the wug has "said" after `ms`: whole characters at
 * `perSecond`, never splitting a surrogate pair. */
export function spoken(text: string, ms: number, perSecond = 70): string {
  const chars = [...text];
  return chars.slice(0, Math.min(chars.length, Math.floor((ms / 1000) * perSecond))).join("");
}
