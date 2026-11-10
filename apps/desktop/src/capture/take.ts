// The life of one recording as data. Every reader (the Media panel, the
// window-close guard, the project-change guard) asks this module, and the
// only way a take is deleted is `discard` after `arm`: the reducer ignores
// any other path to `discarded`.

export type Take =
  | { kind: "idle" }
  | { kind: "starting" }
  | { kind: "recording" }
  | { kind: "stopping" }
  /** The audio is on disk and waits for a name. `message` says why it landed here when it was not the user's Stop. */
  | { kind: "pending"; name: string; message: string | null; armed: boolean }
  | { kind: "saving"; name: string }
  | { kind: "saved" }
  | { kind: "discarded" };

export type TakeEvent =
  | { type: "begin" }
  | { type: "started" }
  | { type: "startFailed" }
  | { type: "stop" }
  | { type: "stopped"; name: string }
  /** The input ended or the upload failed: what was captured is kept. */
  | { type: "interrupted"; reason: string; name: string }
  | { type: "rename"; name: string }
  | { type: "arm" }
  | { type: "disarm" }
  | { type: "save" }
  | { type: "saved" }
  | { type: "saveFailed"; message: string }
  | { type: "discard" }
  /** Leave the take in `recordings-pending/` to come back as a Recovered recording. */
  | { type: "keep" };

export const idle: Take = { kind: "idle" };

const canBegin = (t: Take) => t.kind === "idle" || t.kind === "saved" || t.kind === "discarded";

/** Events that do not fit the current state change nothing: they are late
 * arrivals (an interrupt after Stop, a second click). */
export function step(take: Take, event: TakeEvent): Take {
  switch (event.type) {
    case "begin":
      return canBegin(take) ? { kind: "starting" } : take;
    case "started":
      return take.kind === "starting" ? { kind: "recording" } : take;
    case "startFailed":
      return take.kind === "starting" ? idle : take;
    case "stop":
      return take.kind === "recording" ? { kind: "stopping" } : take;
    case "stopped":
      return take.kind === "stopping" ? { kind: "pending", name: event.name, message: null, armed: false } : take;
    case "interrupted":
      return take.kind === "recording" || take.kind === "stopping" ? { kind: "pending", name: event.name, message: event.reason, armed: false } : take;
    case "rename":
      return take.kind === "pending" ? { ...take, name: event.name } : take;
    case "arm":
      return take.kind === "pending" ? { ...take, armed: true } : take;
    case "disarm":
      return take.kind === "pending" ? { ...take, armed: false } : take;
    case "save":
      return take.kind === "pending" ? { kind: "saving", name: take.name } : take;
    case "saved":
      return take.kind === "saving" ? { kind: "saved" } : take;
    case "saveFailed":
      return take.kind === "saving" ? { kind: "pending", name: take.name, message: event.message, armed: false } : take;
    case "keep":
      return take.kind === "pending" ? idle : take;
    case "discard":
      return take.kind === "pending" && take.armed ? { kind: "discarded" } : take;
  }
}

/** Audio is being captured or waits on disk for the user's decision. */
export const isLive = (take: Take): boolean => take.kind !== "idle" && take.kind !== "saved" && take.kind !== "discarded";

export const CLOSE_BLOCKED = "A recording is in progress, stop it first";
export const PROJECT_CHANGE_BLOCKED = "Finish or discard the recording first";
