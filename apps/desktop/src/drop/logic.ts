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

export function dragAction(drag: { files: boolean; dock: boolean; editable: boolean; card?: boolean }): DragAction {
  if (drag.files) return "import";
  // A media bin card is the Timeline's to take (it carries no link, so nothing to navigate to).
  if (drag.dock || drag.editable || drag.card === true) return "pass";
  return "refuse";
}

const RECORDINGS = new Set(["wav", "wave", "m4a", "mp4", "aac", "mp3", "flac", "ogg", "oga", "aif", "aiff", "aifc"]);
const FILES = new Set(["eaf", "flextext", "lift", "textgrid", "db", "sfm", "lex", "conllu", "conll", "xml", "tei", "txt", "csv", "tsv", "tab", "xlsx", "xls", "xlsm", "ods", "srt", "vtt", "cha", "exb"]);

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
  const detail = "Recordings go to the media bin. ELAN, FLEx, Praat, Toolbox, CSV/TSV and plain text files become texts or lexicon entries.";
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

/** One file of several imported at once: its format, or why it was not. */
export type BatchResult = { name: string; format: string | null; error: string | null };

/** The report for several files imported at once: how many of each format,
 * then each that failed (the first five, then how many more). */
export function batchLines(results: readonly BatchResult[]): string[] {
  const ok = results.filter((r) => r.error === null);
  const byFormat = new Map<string, number>();
  for (const r of ok) byFormat.set(r.format ?? "file", (byFormat.get(r.format ?? "file") ?? 0) + 1);
  const formats = [...byFormat].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).map(([f, n]) => `${n} ${f}`);
  const lines = [`Imported ${ok.length} of ${plural(results.length, "file", "files")}${formats.length > 0 ? `: ${formats.join(", ")}` : ""}.`];
  const failed = results.filter((r) => r.error !== null);
  for (const r of failed.slice(0, 5)) lines.push(`«${r.name}» was not imported: ${r.error}`);
  if (failed.length > 5) lines.push(`${plural(failed.length - 5, "more file was", "more files were")} not imported.`);
  return lines;
}

export const projectChangedReason = "another project was opened after it was dropped.";
export const projectClosedReason = "the project it was dropped into is no longer open.";

export const unsupportedReason =
  "wugs imports recordings (WAV, M4A, MP3, FLAC, Ogg, AIFF), ELAN, FLEx, Praat, Toolbox, CoNLL-U and TEI files (.eaf, .flextext, .lift, .TextGrid, .db, .sfm, .lex, .conllu, .xml), tables of sentences (.csv, .tsv, .xlsx, .ods) and plain text (.txt).";

/** What exporting every text did, for the toast: how many files, and the texts that failed with why. */
export function exportedAllLines(saved: number, total: number, folder: string, failed: readonly { title: string; error: string }[], notes: number): string[] {
  const n = (x: number, one: string, many: string) => `${x} ${x === 1 ? one : many}`;
  const lines = [saved === 0 ? "No text was exported." : `Saved ${n(saved, "text", "texts")} of ${total} in ${folder}.`];
  if (notes > 0) lines.push(`${n(notes, "export has", "exports have")} notes; export a text alone to read them.`);
  for (const f of failed.slice(0, 5)) lines.push(`«${f.title || "untitled"}»: ${f.error}`);
  if (failed.length > 5) lines.push(`and ${failed.length - 5} more could not be exported.`);
  return lines;
}

/** What reading a SayMore project did (`import_saymore`). */
export type SayMoreImport = { sessions: number; transcribed: number; empty: number; recordings: number; details: number; failed: [string, string][]; text: string | null };

/** The toast after a SayMore project was read. */
export function saymoreLines(r: SayMoreImport): string[] {
  const n = (x: number, one: string, many: string) => `${x} ${x === 1 ? one : many}`;
  const lines = [`Read ${n(r.sessions, "SayMore session", "SayMore sessions")}: ${n(r.transcribed, "transcription", "transcriptions")} from their ELAN files${r.empty > 0 ? `, and ${n(r.empty, "new empty text", "new empty texts")} for sessions not transcribed yet` : ""}.`];
  if (r.recordings > 0) lines.push(`${n(r.recordings, "recording was", "recordings were")} imported and linked, so the texts open with their sound.`);
  if (r.details > 0) lines.push(`${n(r.details, "detail", "details")} (date, place, genre, speakers…) written from the session files; see them under the text's Details.`);
  for (const [id, e] of r.failed.slice(0, 5)) lines.push(`${id}: ${e}`);
  if (r.failed.length > 5) lines.push(`and ${r.failed.length - 5} more sessions could not be read.`);
  return lines;
}
