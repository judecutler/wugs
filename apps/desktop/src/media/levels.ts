// What a recording's levels say in words: clipping, a level too low, speech
// too close to the room's noise, a sample rate too low for phonetics.
// Pure, npm-tested; the numbers come from src-tauri/src/levels.rs.
import { duration } from "./logic.ts";

export type Levels = {
  durationMs: number;
  sampleRate: number;
  channels: number;
  peakDb: number | null;
  rmsDb: number | null;
  floorDb: number | null;
  speechDb: number | null;
  clips: number;
  clipTimesMs: number[];
};

/** Below this peak, a recording is quiet enough to lose detail. */
export const QUIET_PEAK = -20;
/** Speech fewer decibels above the room than this is hard to analyse. */
export const NOISY_MARGIN = 20;
/** The lowest sample rate phonetic analysis is comfortable with. */
export const LOW_RATE = 16_000;

/** A level in dBFS, one decimal, with a true minus sign. */
export const dbLabel = (x: number | null): string => (x === null ? "–" : `${x.toFixed(1).replace("-", "−")} dB`);

/** What to look at, in words; empty when nothing stands out. */
export function levelNotes(l: Levels): string[] {
  const notes: string[] = [];
  if (l.clips > 0) {
    const first = l.clipTimesMs[0];
    notes.push(`Clips ${l.clips === 1 ? "once" : `${l.clips} times`}${first === undefined ? "" : `, first at ${duration(first)}`}: the input gain was too high there, and the waveform is cut off.`);
  }
  if (l.peakDb !== null && l.peakDb < QUIET_PEAK) notes.push(`Quiet: it never comes above ${dbLabel(l.peakDb)}. Raise the gain next time; for listening, a copy can be made louder.`);
  if (l.floorDb !== null && l.speechDb !== null && l.speechDb - l.floorDb < NOISY_MARGIN)
    notes.push(`Noisy: speech is only ${Math.round(l.speechDb - l.floorDb)} dB above the room's noise, so pitch and formants will be harder to read.`);
  if (l.sampleRate > 0 && l.sampleRate < LOW_RATE) notes.push(`Recorded at ${l.sampleRate / 1000} kHz, under the 16 kHz phonetic analysis wants.`);
  if (l.peakDb === null) notes.push("Silent: every sample is zero.");
  return notes;
}

/** Every recording's levels as a table to paste. */
export function levelsTsv(rows: readonly { name: string; levels: Levels }[]): string {
  const cell = (x: number | null) => (x === null ? "" : x.toFixed(1));
  return [
    ["Recording", "Length (s)", "Sample rate", "Channels", "Peak dBFS", "Loudness dBFS", "Speech dBFS", "Room dBFS", "Clips", "Notes"].join("\t"),
    ...rows.map(({ name, levels: l }) =>
      [name.replace(/[\t\n]/gu, " "), (l.durationMs / 1000).toFixed(1), String(l.sampleRate), String(l.channels), cell(l.peakDb), cell(l.rmsDb), cell(l.speechDb), cell(l.floorDb), String(l.clips), levelNotes(l).join(" ")].join("\t"),
    ),
  ].join("\n") + "\n";
}
