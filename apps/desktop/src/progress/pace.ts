// How long the work left would take at the recent pace: the day-by-day log's
// last four weeks of hand work (./worklog.ts) against what the Progress
// totals say is still to do. A forecast for planning and grant reports,
// never a promise: it counts working days, not calendar days. Pure.
import type { Progress } from "./logic.ts";
import { shift, type Day } from "./worklog.ts";

/** The window the pace is read over, in days back from today. */
export const WINDOW_DAYS = 28;
/** Fewer working days than this in the window, and there is no pace to speak of. */
export const MIN_DAYS = 3;

/** A day with any hand work (imports and machine suggestions aside). */
export const worked = (d: Day): boolean => d.segments + d.transcribed + d.translated + d.analysed + d.glossed + d.entries + d.senses + d.notes > 0;

export type Pace = {
  /** Working days in the window. */
  days: number;
  /** Per working day. */
  transcribed: number;
  translated: number;
  analysed: number;
  /** Still to do, from the Progress totals. */
  toTranscribe: number;
  toTranslate: number;
  toGloss: number;
};

export function pace(days: readonly Day[], p: Progress, today: string): Pace | null {
  const from = shift(today, -(WINDOW_DAYS - 1));
  const recent = days.filter((d) => d.date >= from && d.date <= today && worked(d));
  if (recent.length < MIN_DAYS) return null;
  const per = (k: "transcribed" | "translated" | "analysed") => recent.reduce((a, d) => a + d[k], 0) / recent.length;
  const sum = (f: (t: Progress["texts"][number]) => number) => p.texts.reduce((a, t) => a + Math.max(0, f(t)), 0);
  return {
    days: recent.length,
    transcribed: per("transcribed"),
    translated: per("translated"),
    analysed: per("analysed"),
    toTranscribe: sum((t) => t.sentences - t.transcribed),
    toTranslate: sum((t) => t.transcribed - t.translated),
    toGloss: sum((t) => t.words - t.glossedWords),
  };
}

const n = (x: number) => Math.round(x).toLocaleString("en");
const rate = (x: number) => (x >= 10 ? n(x) : x.toFixed(1).replace(/\.0$/, ""));
const daysWord = (d: number) => (d < 1 ? "less than a working day" : `about ${n(d)} working ${Math.round(d) === 1 ? "day" : "days"}`);

/** The forecast in words, one line per kind of work with some left; empty when there is no pace yet. */
export function paceLines(pc: Pace | null): string[] {
  if (pc === null) return [];
  const lines = [`At the pace of the last four weeks (${pc.days} working ${pc.days === 1 ? "day" : "days"}):`];
  const one = (left: number, perDay: number, thing: string, done: string) => {
    if (left === 0) return;
    lines.push(perDay > 0 ? `${n(left)} ${thing} still to ${done}: ${daysWord(left / perDay)} (${rate(perDay)} a working day lately).` : `${n(left)} ${thing} still to ${done}, and none were done in those four weeks.`);
  };
  one(pc.toTranscribe, pc.transcribed, pc.toTranscribe === 1 ? "sentence" : "sentences", "transcribe");
  one(pc.toTranslate, pc.translated, pc.toTranslate === 1 ? "sentence" : "sentences", "translate");
  one(pc.toGloss, pc.analysed, pc.toGloss === 1 ? "word" : "words", "gloss");
  return lines.length === 1 ? [] : lines;
}
