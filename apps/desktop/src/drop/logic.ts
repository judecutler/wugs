// Files dropped onto the window: which import each one takes, what the
// overlay says, how a file is cut into pieces for the copy, and the report
// in words. The routing mirrors `src-tauri/src/drops.rs`, which decides for
// real; this copy only keeps an unreadable file from being copied at all.

export type Kind = "recording" | "file" | "unsupported";

/** What the window does with a drag. Files are imported; a dock tab drag
 * and a drag over a text field go on to them; anything else (a link or
 * text from another program) is refused, so the webview never opens it as
 * a page. */
export type DragAction = "import" | "pass" | "refuse";

export function dragAction(drag: { files: boolean; dock: boolean; editable: boolean }): DragAction {
  if (drag.files) return "import";
  if (drag.dock || drag.editable) return "pass";
  return "refuse";
}

const RECORDINGS = new Set(["wav", "wave", "m4a", "mp4", "aac", "mp3", "flac", "ogg", "oga", "aif", "aiff", "aifc"]);
const FILES = new Set(["eaf", "flextext", "lift", "textgrid", "db", "sfm", "lex", "txt"]);

/** Pieces of this size keep memory flat for a two-hour WAV and the progress moving. */
export const CHUNK_BYTES = 8 * 1024 * 1024;

export function classify(name: string): Kind {
  const dot = name.lastIndexOf(".");
  const ext = dot < 0 ? "" : name.slice(dot + 1).toLowerCase();
  if (RECORDINGS.has(ext)) return "recording";
  if (FILES.has(ext)) return "file";
  return "unsupported";
}

/** What is being dragged, as far as the webview tells before the drop: a
 * count and how many say they are audio. Some webviews tell nothing until
 * the drop (`count` 0). */
export type Dragged = { count: number; audio: number };

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

export function overlayWords(projectOpen: boolean, dragged: Dragged): { title: string; detail: string } {
  if (!projectOpen) return { title: "Open a project first", detail: "Dropped files go into the open project. Nothing is imported now." };
  const detail = "Recordings go to the media bin. ELAN, FLEx, Praat and Toolbox files become texts or lexicon entries.";
  if (dragged.count === 0) return { title: "Drop to import", detail };
  if (dragged.audio === dragged.count) return { title: `Drop to import ${plural(dragged.count, "recording", "recordings")}`, detail };
  return { title: `Drop to import ${plural(dragged.count, "file", "files")}`, detail };
}

/** `[start, end)` byte ranges covering a file of `size` bytes, in order. */
export function chunks(size: number, chunkBytes = CHUNK_BYTES): [number, number][] {
  const out: [number, number][] = [];
  for (let start = 0; start < size; start += chunkBytes) out.push([start, Math.min(size, start + chunkBytes)]);
  return out;
}

function amount(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(0)} MB`;
  return `${Math.max(1, Math.round(bytes / 1e3))} kB`;
}

export function progressLine(name: string, sent: number, total: number, waiting: number): string {
  const share = total === 0 ? 100 : Math.floor((sent / total) * 100);
  const more = waiting > 0 ? ` (${plural(waiting, "more file", "more files")} after this)` : "";
  if (sent >= total) return `Importing «${name}»…${more}`;
  return `Copying «${name}» into the project: ${share}% of ${amount(total)}${more}`;
}

/** How one dropped file went. `text`: the text to show afterwards; `lexicon`: the lexicon changed. */
export type Outcome =
  | { kind: "recording" }
  | { kind: "file"; format: string; lines: string[]; text: string | null; lexicon: boolean }
  | { kind: "refused"; reason: string };

export function outcomeLines(name: string, outcome: Outcome): string[] {
  switch (outcome.kind) {
    case "recording":
      return [`«${name}» is in the media bin.`];
    case "file":
      return [`Imported «${name}» (${outcome.format}).`, ...outcome.lines];
    case "refused":
      return [`«${name}» was not imported: ${outcome.reason}`];
  }
}

export const projectChangedReason = "another project was opened after it was dropped.";
export const projectClosedReason = "the project it was dropped into is no longer open.";

export const unsupportedReason =
  "wugs imports recordings (WAV, M4A, MP3, FLAC, Ogg, AIFF) ELAN, FLEx, Praat and Toolbox files (.eaf, .flextext, .lift, .TextGrid, .db, .sfm, .lex) and plain text (.txt).";
