// Speakers: each transcription track's utterance lengths (src-tauri/src/mlu.rs),
// the mean length of utterance child language research starts from. Pure.

export type SpeakerRow = { tier: string; name: string; texts: number; utterances: number; words: number; types: number; longest: number; analysed: number; morphemes: number };

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

/** One row of the table, in words: speaker, texts, utterances, words, MLU (words), MLU (morphemes), Brown's stage, longest, different words. */
export function speakerCells(r: SpeakerRow): string[] {
  const m = mluMorphemes(r);
  const part = m === null ? "–" : r.analysed < r.utterances ? `${fixed(m)} (${r.analysed} of ${r.utterances})` : fixed(m);
  const stage = m !== null && isChild(r.name) ? brownStage(m) : "";
  return [r.name || "(no name)", String(r.texts), String(r.utterances), String(r.words), fixed(mluWords(r)), part, stage, String(r.longest), String(r.types)];
}

export const SPEAKER_COLUMNS = ["Speaker", "Texts", "Utterances", "Words", "MLU (words)", "MLU (morphemes)", "Brown's stage", "Longest", "Different words"];

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
  if (rows.some((r) => isChild(r.name) && r.analysed > 0)) parts.push("Brown's stages are for a child's track (CHI) and were set for English; read them as a rough guide in other languages.");
  return parts.join(" ");
}

export function speakersTsv(rows: readonly SpeakerRow[]): string {
  return [SPEAKER_COLUMNS.join("\t"), ...rows.map((r) => speakerCells(r).join("\t"))].join("\n") + "\n";
}
