// Speakers: each transcription track's utterance lengths (src-tauri/src/mlu.rs),
// the mean length of utterance child language research starts from. Pure.

import type { TextLength } from "./mluage.ts";

export type SpeakerRow = { tier: string; name: string; texts: number; utterances: number; words: number; types: number; longest: number; analysed: number; morphemes: number; mtld: number | null; vocd: number | null; person: string; sex: string; age: string; born: string; byText: readonly TextLength[] };

/** Words per utterance. */
export const mluWords = (r: SpeakerRow): number => (r.utterances === 0 ? 0 : r.words / r.utterances);

/** Morphemes per utterance, over the utterances whose morphemes are known; null with none. */
export const mluMorphemes = (r: SpeakerRow): number | null => (r.analysed === 0 ? null : r.morphemes / r.analysed);

/** Brown's (1973) stages by MLU in morphemes: I below 2.0, II to 2.5, III to 3.0, IV to 3.75, V to 4.5, then past V. */
export function brownStage(mlu: number): string {
  if (mlu < 2) return "I";
  if (mlu < 2.5) return "II";
  if (mlu < 3) return "III";
  if (mlu < 3.75) return "IV";
  if (mlu < 4.5) return "V";
  return "past V";
}

/** A track that holds a child's speech: CHAT's `CHI`, or a name with "child" in it. */
export const isChild = (name: string): boolean => /^chi\d*$/iu.test(name.trim()) || /child/iu.test(name);

const fixed = (n: number) => n.toFixed(2);

/** Whether any track has a speaker set in the People panel, so the table gives who they are. */
export const hasPeople = (rows: readonly SpeakerRow[]): boolean => rows.some((r) => r.person !== "" || r.sex !== "" || r.age !== "");

/** One row of the table, in words: speaker, (with `people`: spoken by, sex, age,) texts, utterances, words, MLU (words), MLU (morphemes), Brown's stage, longest, different words, MTLD, vocd-D. */
export function speakerCells(r: SpeakerRow, people = false): string[] {
  const m = mluMorphemes(r);
  const part = m === null ? "–" : r.analysed < r.utterances ? `${fixed(m)} (${r.analysed} of ${r.utterances})` : fixed(m);
  const stage = m !== null && isChild(r.name) ? brownStage(m) : "";
  const who = people ? [r.person, r.sex, r.age] : [];
  return [r.name || "(no name)", ...who, String(r.texts), String(r.utterances), String(r.words), fixed(mluWords(r)), part, stage, String(r.longest), String(r.types), r.mtld === null ? "–" : r.mtld.toFixed(1), r.vocd === null ? "–" : r.vocd.toFixed(1)];
}

const PEOPLE_COLUMNS = ["Spoken by", "Sex", "Age"];

/** The table's columns, with who each speaker is when any track has one set. */
export const speakerColumns = (rows: readonly SpeakerRow[]): string[] => (hasPeople(rows) ? [SPEAKER_COLUMNS[0]!, ...PEOPLE_COLUMNS, ...SPEAKER_COLUMNS.slice(1)] : [...SPEAKER_COLUMNS]);

export const SPEAKER_COLUMNS = ["Speaker", "Texts", "Utterances", "Words", "MLU (words)", "MLU (morphemes)", "Brown's stage", "Longest", "Different words", "MTLD", "vocd-D"];

/** The note under the table. */
export function speakersNote(rows: readonly SpeakerRow[]): string {
  if (rows.length === 0) return "";
  const parts = ["Each transcription track is a speaker; CHAT files come in a track per speaker."];
  const some = rows.some((r) => r.analysed > 0);
  parts.push(
    some
      ? "Morphemes are counted in glossed sentences (every word broken into morphemes) and in CHAT's %mor tier, as CLAN counts them; other utterances are left out of that measure."
      : "Gloss the sentences, or import CHAT with a %mor tier, to count morphemes too.",
  );
  parts.push("MTLD and vocd-D measure how varied the words are without growing with how much was said, so speakers recorded for different lengths compare; MTLD needs 100 words or more, vocd-D 50 (CLAN's vocd, from random samples of 35 to 50 words, so it differs from CLAN's own by chance a little).");
  if (!hasPeople(rows)) parts.push("Set who speaks on a track (double-click its name on the Timeline) to list their sex and age from the People panel here.");
  if (rows.some((r) => isChild(r.name) && r.analysed > 0)) parts.push("Brown's stages are for a child's track (CHI) and were set for English; read them as a rough guide in other languages.");
  return parts.join(" ");
}

export function speakersTsv(rows: readonly SpeakerRow[]): string {
  const people = hasPeople(rows);
  return [speakerColumns(rows).join("\t"), ...rows.map((r) => speakerCells(r, people).join("\t"))].join("\n") + "\n";
}
