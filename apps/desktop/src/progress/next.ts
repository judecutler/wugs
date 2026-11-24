// What to do next: the project's open work in the order fieldwork usually
// takes it (transcribe, translate, gloss, then the lexicon and the records),
// each with the panel that does it. Read from the Progress figures, the
// lexicon list and the consent records. Pure: no DOM.

import type { Progress } from "./logic.ts";

/** Which Lexicon list a step opens (the Lexicon panel's `ListShow`). */
export type LexiconList = "gloss" | "category" | "example" | "sound";

export type Step = { id: string; count: number; text: string; panel: string; button: string; list?: LexiconList };

/** How many entries each Lexicon list holds, from the panel's own rule. */
export type LexiconGaps = Partial<Record<LexiconList, number>>;

const n = (x: number, one: string, many: string) => `${x.toLocaleString("en")} ${x === 1 ? one : many}`;

export function nextSteps(p: Progress, lexicon: LexiconGaps, uncoveredTexts: number | null): Step[] {
  const sum = (f: (t: Progress["texts"][number]) => number) => p.texts.reduce((a, t) => a + Math.max(0, f(t)), 0);
  const steps: Step[] = [];
  const add = (s: Step) => {
    if (s.count > 0) steps.push(s);
  };
  const untranscribed = sum((t) => t.sentences - t.transcribed);
  add({ id: "transcribe", count: untranscribed, text: `${n(untranscribed, "segment has", "segments have")} no transcription yet.`, panel: "transcribe", button: "Transcribe" });
  const untranslated = sum((t) => t.transcribed - t.translated);
  add({ id: "translate", count: untranslated, text: `${n(untranslated, "transcribed sentence has", "transcribed sentences have")} no translation.`, panel: "interlinear", button: "Interlinear" });
  const unglossed = sum((t) => t.words - t.glossedWords);
  add({ id: "gloss", count: unglossed, text: `${n(unglossed, "word in the texts is", "words in the texts are")} not glossed yet; Words to gloss takes the commonest first.`, panel: "wordtypes", button: "Words to gloss" });
  const lex: [LexiconList, string][] = [
    ["gloss", "has a sense with no gloss"],
    ["category", "has no category"],
    ["example", "has a sense with no example sentence"],
    ["sound", "is not heard in a recording yet"],
  ];
  for (const [list, what] of lex) {
    const c = lexicon[list] ?? 0;
    add({ id: `lexicon-${list}`, count: c, text: `${n(c, "lexicon entry", "lexicon entries")} ${c === 1 ? what : what.replace(/^has /, "have ").replace(/^is /, "are ")}.`, panel: "lexicon", button: "Lexicon", list });
  }
  if (uncoveredTexts !== null) add({ id: "consent", count: uncoveredTexts, text: `${n(uncoveredTexts, "text has", "texts have")} no consent record covering it.`, panel: "people", button: "People" });
  return steps;
}

/** The line over the steps. */
export function stepsLine(steps: readonly Step[]): string {
  return steps.length === 0 ? "Nothing waits: every segment is transcribed, translated and glossed, every entry is complete, and every text has a consent record." : "In the order fieldwork usually takes it. Each button opens the panel that does it.";
}
