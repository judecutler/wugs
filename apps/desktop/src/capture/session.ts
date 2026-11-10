// Owns the live recorder and the take's state for the whole window. The
// Media panel only renders this, so closing or reopening the panel never
// touches a recording, and the window-close and project-change guards ask
// the same state.

import { errorText } from "../api";
import { refreshAll, showToast } from "../store";
import { defaultLabel, type Level } from "./pcm.ts";
import { startRecording, type Recorder } from "./recorder.ts";
import { CLOSE_BLOCKED, idle, isLive, PROJECT_CHANGE_BLOCKED, step, type Take, type TakeEvent } from "./take.ts";

let take: Take = idle;
let recorder: Recorder | null = null;
let level: Level = { peak: 0, rms: 0 };
let startError: string | null = null;
const listeners = new Set<() => void>();

export const getTake = (): Take => take;
export const takeLevel = (): Level => level;
export const takeElapsedMs = (): number => recorder?.elapsedMs() ?? 0;
export const takeStartError = (): string | null => startError;
export const decayLevel = () => (level = { peak: level.peak * 0.9, rms: level.rms * 0.9 });

export function subscribeTake(f: () => void): () => void {
  listeners.add(f);
  return () => listeners.delete(f);
}

function dispatch(event: TakeEvent) {
  const next = step(take, event);
  if (next === take) return;
  take = next;
  if (!isLive(take)) recorder = null;
  for (const f of listeners) f();
}

/** True, with a toast, when a live take means the project must stay open. */
export function refuseProjectChange(): boolean {
  if (!isLive(take)) return false;
  showToast({ tone: "warn", lines: [PROJECT_CHANGE_BLOCKED] });
  return true;
}

/** The message the window-close guard shows, or null when closing is safe. */
export const closeBlocker = (): string | null => (isLive(take) ? CLOSE_BLOCKED : null);

export async function startTake(deviceId: string | null) {
  dispatch({ type: "begin" });
  if (take.kind !== "starting") return;
  startError = null;
  try {
    recorder = await startRecording({
      deviceId,
      onLevel: (l) => (level = l),
      onInterrupted: (reason) => dispatch({ type: "interrupted", reason, name: defaultLabel(new Date()) }),
    });
    dispatch({ type: "started" });
  } catch (e) {
    startError = e instanceof Error ? e.message : String(e);
    dispatch({ type: "startFailed" });
  }
}

export async function stopTake() {
  const rec = recorder;
  if (rec === null || take.kind !== "recording") return;
  dispatch({ type: "stop" });
  const reason = await rec.stop();
  const name = defaultLabel(new Date());
  dispatch(reason === null ? { type: "stopped", name } : { type: "interrupted", reason, name });
}

export const renameTake = (name: string) => dispatch({ type: "rename", name });
export const armDiscard = () => dispatch({ type: "arm" });
export const disarmDiscard = () => dispatch({ type: "disarm" });

export async function saveTake() {
  const rec = recorder;
  if (rec === null || take.kind !== "pending") return;
  const name = take.name;
  dispatch({ type: "save" });
  try {
    await rec.finish(name);
    dispatch({ type: "saved" });
    showToast({ tone: "info", lines: ["Recording saved to the media bin."] });
    await refreshAll();
  } catch (e) {
    // The file stays in recordings-pending/, so Save can retry.
    dispatch({ type: "saveFailed", message: errorText(e) });
  }
}

/** Deletes the take. A no-op unless the user armed the confirmation. */
export async function discardTake() {
  const rec = recorder;
  if (rec === null || take.kind !== "pending" || !take.armed) return;
  try {
    await rec.discard();
    dispatch({ type: "discard" });
  } catch (e) {
    dispatch({ type: "disarm" });
    showToast({ tone: "error", lines: [`Could not discard the recording: ${errorText(e)}`] });
  }
}

/** Leaves the take on disk for a Recovered recording, freeing the window to change project. */
export async function keepTake() {
  const rec = recorder;
  if (rec === null || take.kind !== "pending") return;
  try {
    await rec.keep();
    dispatch({ type: "keep" });
    showToast({ tone: "info", lines: ["Recording kept. It appears under Recovered recordings when its project is open."] });
  } catch (e) {
    showToast({ tone: "error", lines: [`Could not keep the recording: ${errorText(e)}`] });
  }
}
