// The whole window is one drop target for files. Native drag-and-drop is
// off in tauri.conf.json (`dragDropEnabled: false`): on Windows it would
// swallow every HTML5 drag, dock tabs included. With it off, the webview
// would also open a dropped file or link as a page, so every drag is sorted
// here first (`dragAction`): files are imported and kept from dockview, a
// dock tab drag, a media bin card being dragged (a flag only an in-app
// dragstart sets) or a drag over a text field goes on untouched, and
// anything else is refused. The navigation guard in lib.rs is the second layer.
import { dropBegin, dropCancel, dropChunk, dropFinish, errorText } from "../api";
import { h } from "../dom";
import { cardDrag } from "../media/logic";
import { getPaneData, getPanelData } from "../dockview";
import { getState, refreshAll, selectText, showToast, touchLexicon, wantMedia } from "../store";
import {
  chunks,
  classify,
  dragAction,
  outcomeLines,
  overlayWords,
  progressLine,
  projectChangedReason,
  projectClosedReason,
  unsupportedReason,
  type DragAction,
  type Dragged,
  type Outcome,
} from "./logic";

// The spec lets a still drag repeat dragover only every 350 ms ± 200 ms
// (WebKit keeps to it), so a shorter silence would flicker the overlay.
const QUIET_MS = 1000;

const TEXT_INPUTS = new Set(["text", "search", "url", "email", "tel", "number", "password"]);

/** Where dropped text lands as text; a checkbox, button or read-only field would let the webview open a link instead. */
function takesText(target: EventTarget | null): boolean {
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLInputElement) {
    const typed = target instanceof HTMLTextAreaElement || TEXT_INPUTS.has(target.type);
    return typed && !target.readOnly && !target.disabled;
  }
  return target instanceof HTMLElement && target.isContentEditable;
}

function action(e: DragEvent): DragAction {
  return dragAction({
    files: e.dataTransfer?.types.includes("Files") ?? false,
    dock: getPanelData() !== undefined || getPaneData() !== undefined,
    editable: takesText(e.target),
    card: cardDrag.active(),
  });
}

function dragged(e: DragEvent): Dragged {
  const items = [...(e.dataTransfer?.items ?? [])].filter((i) => i.kind === "file");
  return { count: items.length, audio: items.filter((i) => i.type.startsWith("audio/")).length };
}

/** Keep a drag from the webview's default (opening it as a page) and from dockview. */
function claim(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
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
      const act = action(e);
      if (act === "pass") return;
      claim(e);
      if (act === "import") show(e);
      else if (e.dataTransfer !== null) e.dataTransfer.dropEffect = "none";
    },
    true,
  );
  window.addEventListener(
    "dragover",
    (e) => {
      const act = action(e);
      if (act === "pass") return;
      claim(e);
      // `none` and the drop never arrives: a refused drag, or files with no
      // project open (the overlay says why).
      if (e.dataTransfer !== null) e.dataTransfer.dropEffect = act === "import" && getState().kind === "open" ? "copy" : "none";
      if (act === "refuse") return;
      if (overlay.hidden) show(e);
      window.clearTimeout(quiet);
      quiet = window.setTimeout(hide, QUIET_MS);
    },
    true,
  );
  window.addEventListener(
    "dragleave",
    (e) => {
      if (action(e) !== "import") return;
      e.stopPropagation();
      if (e.relatedTarget === null) hide();
    },
    true,
  );
  window.addEventListener(
    "drop",
    (e) => {
      const act = action(e);
      if (act === "pass") return;
      claim(e);
      if (act === "refuse") return;
      hide();
      const state = getState();
      const files = [...(e.dataTransfer?.files ?? [])];
      if (files.length === 0 || state.kind !== "open") return;
      queue.push(...files.map((file) => ({ file, project: state.info.path })));
      void drain();
    },
    true,
  );
  return overlay;
}

/** A dropped file and the project (its folder) that was open when it was dropped. */
type Queued = { file: File; project: string };

const queue: Queued[] = [];
let draining = false;

const openProject = (): string | null => {
  const state = getState();
  return state.kind === "open" ? state.info.path : null;
};

/** One file after another; a drop during a run joins the queue. Never rejects. */
async function drain() {
  if (draining) return;
  draining = true;
  const report: string[] = [];
  let refused = 0;
  let total = 0;
  let lastText: { id: string; project: string } | null = null;
  let lexicon = false;
  try {
    while (queue.length > 0) {
      const { file, project } = queue.shift()!;
      total += 1;
      const progress = (sent: number) => showToast({ tone: "info", lines: [...report, progressLine(file.name, sent, file.size, queue.length)] });
      const now = openProject();
      const outcome: Outcome =
        now === project ? await importOne(file, project, progress) : { kind: "refused", reason: now === null ? projectClosedReason : projectChangedReason };
      if (outcome.kind === "refused") refused += 1;
      if (outcome.kind === "file") {
        // Only a text of the project still open can be selected afterwards.
        if (outcome.text != null) lastText = { id: outcome.text, project };
        lexicon ||= outcome.lexicon;
      }
      report.push(...outcomeLines(file.name, outcome));
    }
  } finally {
    draining = false;
  }
  // Refresh first (it reports its own failures), then the report, so no
  // other toast can replace what was and was not imported.
  try {
    if (lexicon) touchLexicon();
    await refreshAll();
    if (lastText !== null && openProject() === lastText.project) await selectText(lastText.id);
  } catch (e) {
    report.push(`The panels could not be refreshed: ${errorText(e)}`);
  }
  showToast({ tone: refused === 0 ? "info" : refused === total ? "error" : "warn", lines: report });
}

async function importOne(file: File, project: string, progress: (sent: number) => void): Promise<Outcome> {
  if (classify(file.name) === "unsupported") return { kind: "refused", reason: unsupportedReason };
  let dropId: string | null = null;
  try {
    progress(0);
    dropId = await dropBegin({ project, name: file.name, size: file.size });
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
