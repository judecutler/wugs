// A speaker's utterance length session by session against their age: the
// longitudinal view child language studies chart (Brown 1973), read from
// each text's CHAT age (`age_chi`) or worked out from the text's date and
// the person's birth date.
import { brownStage, type SpeakerRow } from "./speakers.ts";

export type TextLength = { text: string; title: string; age: string; date: string; utterances: number; words: number; analysed: number; morphemes: number };
export type AgePoint = { text: string; title: string; months: number; utterances: number; mluWords: number; mluMorphemes: number | null };

/** An age as CHAT writes it (`2;06.15` years;months.days, `3;` or `2;6`) or a plain number of years, in months; null otherwise. */
export function chatAgeMonths(age: string): number | null {
  const a = age.trim();
  const chat = /^(\d{1,3});(\d{1,2})?(?:\.(\d{1,2}))?$/u.exec(a);
  if (chat !== null) {
    const months = Number(chat[2] ?? 0);
    const days = Number(chat[3] ?? 0);
    if (months > 11 || days > 31) return null;
    return Number(chat[1]) * 12 + months + days / 30.4375;
  }
  return /^\d{1,3}$/u.test(a) ? Number(a) * 12 : null;
}

const day = (s: string): number | null => {
  const m = /^(\d{4})-(\d{2})(?:-(\d{2}))?$/u.exec(s.trim());
  if (m === null) return null;
  const t = Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3] ?? 1));
  return Number.isNaN(t) ? null : t;
};

/** The months from a birth date to a recording date (both `YYYY-MM-DD` or `YYYY-MM`); null when either is missing or the order is wrong. */
export function monthsBetween(born: string, date: string): number | null {
  const a = day(born);
  const b = day(date);
  if (a === null || b === null || b < a) return null;
  return (b - a) / (86_400_000 * 30.4375);
}

/** A text's age for the speaker: its own CHAT age, else its date against their birth date. */
export const ageMonths = (t: TextLength, born: string): number | null => chatAgeMonths(t.age) ?? monthsBetween(born, t.date);

/** The texts in which the speaker's age is known, youngest first, each with its MLU. */
export function agePoints(row: Pick<SpeakerRow, "born" | "byText">): AgePoint[] {
  return row.byText
    .flatMap((t): AgePoint[] => {
      const months = ageMonths(t, row.born);
      if (months === null || t.utterances === 0) return [];
      return [{ text: t.text, title: t.title, months, utterances: t.utterances, mluWords: t.words / t.utterances, mluMorphemes: t.analysed === 0 ? null : t.morphemes / t.analysed }];
    })
    .sort((a, b) => a.months - b.months || a.title.localeCompare(b.title));
}

/** Whether the chart shows MLU in morphemes: every point has them. */
export const inMorphemes = (points: readonly AgePoint[]): boolean => points.length > 0 && points.every((p) => p.mluMorphemes !== null);

/** The value plotted for a point. */
export const plotted = (p: AgePoint, morphemes: boolean): number => (morphemes ? (p.mluMorphemes ?? 0) : p.mluWords);

/** Months as CHAT writes an age: `2;06`. */
export function ageLabel(months: number): string {
  const whole = Math.floor(months + 1e-9);
  return `${Math.floor(whole / 12)};${String(whole % 12).padStart(2, "0")}`;
}

/** Tick marks along the age axis: every 1, 3, 6 or 12 months, whichever gives at most 8. */
export function ageTicks(from: number, to: number): number[] {
  const step = [1, 3, 6, 12, 24, 60].find((s) => (to - from) / s <= 8) ?? 120;
  const out: number[] = [];
  for (let m = Math.ceil(from / step) * step; m <= to + 1e-9; m += step) out.push(m);
  return out;
}

/** The note under the chart, in words. */
export function ageNote(name: string, points: readonly AgePoint[], child: boolean): string {
  if (points.length < 2) return "";
  const morphemes = inMorphemes(points);
  const first = points[0]!;
  const last = points[points.length - 1]!;
  const unit = morphemes ? "morphemes" : "words";
  const a = plotted(first, morphemes);
  const b = plotted(last, morphemes);
  const way = b > a + 0.05 ? "rose" : b < a - 0.05 ? "fell" : "stayed";
  const line = way === "stayed" ? `${name}'s MLU in ${unit} stayed near ${b.toFixed(2)}` : `${name}'s MLU in ${unit} ${way} from ${a.toFixed(2)} at ${ageLabel(first.months)} to ${b.toFixed(2)} at ${ageLabel(last.months)}`;
  const stage = morphemes && child ? ` (Brown's stage ${brownStage(a)} to ${brownStage(b)})` : "";
  const small = points.filter((p) => p.utterances < 50).length;
  const caution = small === 0 ? "" : ` ${small === 1 ? "One session has" : `${small} sessions have`} fewer than 50 utterances, so ${small === 1 ? "its" : "their"} MLU is rough; Brown counted 100.`;
  return `${line} over ${points.length} sessions${stage}.${caution}`;
}

export function ageTsv(points: readonly AgePoint[]): string {
  const head = ["text", "age", "months", "utterances", "mlu_words", "mlu_morphemes"].join("\t");
  return [head, ...points.map((p) => [p.title, ageLabel(p.months), p.months.toFixed(1), String(p.utterances), p.mluWords.toFixed(2), p.mluMorphemes === null ? "" : p.mluMorphemes.toFixed(2)].join("\t"))].join("\n") + "\n";
}
