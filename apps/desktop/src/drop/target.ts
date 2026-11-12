// The whole window is one drop target for files. Only drags that carry
// files are touched (a dockview tab drag carries none), and those are kept
// from reaching dockview, so neither can take the other's drag. Native
// drag-and-drop is off in tauri.conf.json (`dragDropEnabled: false`): on
// Windows it would swallow every HTML5 drag, tabs included.
import { dropBegin, dropCancel, dropChunk, dropFinish, errorText } from "../api";
import { h } from "../dom";
import { getState, refreshAll, selectText, showToast, touchLexicon, wantMedia } from "../store";
import { chunks, classify, outcomeLines, overlayWords, progressLine, unsupportedReason, type Dragged, type Outcome } from "./logic";

// The spec lets a still drag repeat dragover only every 350 ms ± 200 ms
// (WebKit keeps to it), so a shorter silence would flicker the overlay.
const QUIET_MS = 1000;

const carriesFiles = (e: DragEvent) => e.dataTransfer?.types.includes("Files") ?? false;

function dragged(e: DragEvent): Dragged {
  const items = [...(e.dataTransfer?.items ?? [])].filter((i) => i.kind === "file");
  return { count: items.length, audio: items.filter((i) => i.type.startsWith("audio/")).length };
}

/** Mounts the overlay and the window listeners; returns the overlay to place in the page. */
export function dropTarget(): HTMLElement {
  const title = h("div", { className: "drop-title" });
  const detail = h("div", { className: "drop-detail" });
  const overlay = h("div", { className: "drop-overlay", hidden: true }, h("div", { className: "drop-box" }, title, detail));
  // Leaving the window is not reported the same way by every webview, and
  // an Escape mid-drag may report nothing; dragover repeats while a drag is
  // over the window, so its silence hides the overlay.
  let quiet = 0;
  const hide = () => {
    overlay.hidden = true;
    window.clearTimeout(quiet);
  };
  const show = (e: DragEvent) => {
    const words = overlayWords(getState().kind === "open", dragged(e));
    title.textContent = words.title;
    detail.textContent = words.detail;
    overlay.hidden = false;
    window.clearTimeout(quiet);
    quiet = window.setTimeout(hide, QUIET_MS);
  };

  window.addEventListener(
    "dragenter",
    (e) => {
      if (!carriesFiles(e)) return;
      e.preventDefault();
      e.stopPropagation();
      show(e);
    },
    true,
  );
  window.addEventListener(
    "dragover",
    (e) => {
      if (!carriesFiles(e)) return;
      e.preventDefault();
      e.stopPropagation();
      if (e.dataTransfer !== null) e.dataTransfer.dropEffect = getState().kind === "open" ? "copy" : "none";
      if (overlay.hidden) show(e);
      window.clearTimeout(quiet);
      quiet = window.setTimeout(hide, QUIET_MS);
    },
    true,
  );
  window.addEventListener(
    "dragleave",
    (e) => {
      if (!carriesFiles(e)) return;
      e.stopPropagation();
      if (e.relatedTarget === null) hide();
    },
    true,
  );
  window.addEventListener(
    "drop",
    (e) => {
      if (!carriesFiles(e)) return;
      // Never let the webview open the file as a page.
      e.preventDefault();
      e.stopPropagation();
      hide();
      const files = [...(e.dataTransfer?.files ?? [])];
      if (files.length === 0) return;
      if (getState().kind !== "open") {
        showToast({ tone: "warn", lines: ["Open a project first, then drop the files again. Nothing was imported."] });
        return;
      }
      queue.push(...files);
      void drain();
    },
    true,
  );
  return overlay;
}

const queue: File[] = [];
let draining = false;

/** One file after another; a drop during a run joins the queue. */
async function drain() {
  if (draining) return;
  draining = true;
  const report: string[] = [];
  let refused = 0;
  let total = 0;
  let lastText: string | null = null;
  let lexicon = false;
  while (queue.length > 0) {
    const file = queue.shift()!;
    total += 1;
    const progress = (sent: number) => showToast({ tone: "info", lines: [...report, progressLine(file.name, sent, file.size, queue.length)] });
    const outcome = await importOne(file, progress);
    if (outcome.kind === "refused") refused += 1;
    if (outcome.kind === "file") {
      lastText = outcome.text ?? lastText;
      lexicon ||= outcome.lexicon;
    }
    report.push(...outcomeLines(file.name, outcome));
  }
  draining = false;
  // The report first: a failed refresh below must not hide what was imported.
  showToast({ tone: refused === 0 ? "info" : refused === total ? "error" : "warn", lines: report });
  if (lexicon) touchLexicon();
  await refreshAll();
  if (lastText !== null) await selectText(lastText);
}

async function importOne(file: File, progress: (sent: number) => void): Promise<Outcome> {
  if (classify(file.name) === "unsupported") return { kind: "refused", reason: unsupportedReason };
  let dropId: string | null = null;
  try {
    progress(0);
    dropId = await dropBegin({ name: file.name, size: file.size });
    for (const [start, end] of chunks(file.size)) {
      await dropChunk(dropId, new Uint8Array(await file.slice(start, end).arrayBuffer()));
      progress(end);
    }
    const id = dropId;
    dropId = null;
    const r = await dropFinish({ dropId: id });
    if (r.kind === "recording") {
      wantMedia(r.source.id);
      return { kind: "recording" };
    }
    return { kind: "file", format: r.imported.format, lines: r.imported.lines, text: r.imported.text, lexicon: r.imported.lexicon };
  } catch (e) {
    // A file that cannot be read (moved, or a folder) fails here too.
    if (dropId !== null) void dropCancel({ dropId }).catch(() => {});
    return { kind: "refused", reason: errorText(e) };
  }
}
