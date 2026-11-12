// Words and checks for Send feedback (roadmap 65), kept apart from the DOM.

export const MAX_MESSAGE_CHARS = 10_000;

export type System = "windows" | "mac" | "linux" | "other";

export type Outcome = { sent: boolean; waiting: number; problem: string | null };

export function systemOf(hint: string): System {
  if (/win/i.test(hint)) return "windows";
  if (/mac/i.test(hint)) return "mac";
  if (/linux|x11/i.test(hint)) return "linux";
  return "other";
}

/** How to record a short video of the screen with what the computer already has. */
export function recordingTip(system: System): string {
  switch (system) {
    case "windows":
      return "To record the screen, press Windows + Shift + R (or open the Snipping Tool and choose Record). The video is saved in Videos › Screen Recordings.";
    case "mac":
      return "To record the screen, press Shift + Command + 5 and choose Record Selected Portion; stop it from the menu bar. The video is saved on your Desktop.";
    case "linux":
      return "To record the screen on most Linux desktops, press Ctrl + Shift + Alt + R to start and again to stop. The video is saved in Videos.";
    default:
      return "A short screen recording or a screenshot helps.";
  }
}

/** Why the Send button is off, or null when the form can be sent. */
export function sendBlocker(message: string, contact: string): string | null {
  if (message.trim() === "") return "Write what happened first.";
  if (message.length > MAX_MESSAGE_CHARS) return `Keep it under ${MAX_MESSAGE_CHARS.toLocaleString("en")} characters.`;
  const c = contact.trim();
  if (c !== "" && (!c.includes("@") || /\s/.test(c))) return "The reply address does not look like an email address.";
  return null;
}

/** The last part of a path, for showing which file is attached. */
export const fileName = (path: string): string => path.split(/[\\/]/).pop() ?? path;

/** The toast after a send, in words. */
export function outcomeLines(o: Outcome): { tone: "info" | "warn"; lines: string[] } {
  if (o.sent) {
    const more = o.waiting > 0 ? [`${o.waiting} earlier ${o.waiting === 1 ? "report is" : "reports are"} still waiting and will go when the connection allows.`] : [];
    return { tone: "info", lines: ["Thank you. Your feedback was sent.", ...more] };
  }
  if (o.waiting > 0) {
    return { tone: "warn", lines: ["Your feedback is saved on this computer and will be sent when wugs can reach the internet.", ...(o.problem ? [o.problem] : [])] };
  }
  return { tone: "warn", lines: ["Your feedback could not be sent.", ...(o.problem ? [o.problem] : [])] };
}
