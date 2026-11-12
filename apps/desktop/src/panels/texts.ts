import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { createText, errorText, exportLexicon, exportText, importFile, sealProject, type LexiconFormat, type TextFormat } from "../api";
import { megabytes } from "../assist/logic";
import { localDay } from "../dates";
import { emptyState, h } from "../dom";
import { attempt, getState, reloadTexts, selectText, showToast, touchLexicon } from "../store";
import { sliced, type PanelDef } from "./panel";

async function newText(title: string) {
  if (title === "") return;
  await attempt(async () => {
    const textId = await createText({ title });
    await reloadTexts();
    await selectText(textId);
  });
}

const TEXT_FORMATS: [TextFormat, string][] = [
  ["elan", "ELAN (.eaf)"],
  ["flextext", "FLEx (.flextext)"],
  ["textgrid", "Praat (.TextGrid)"],
  ["plaintext", "Plain text, a line per segment (.txt)"],
];
const LEXICON_FORMATS: [LexiconFormat, string][] = [
  ["lift", "LIFT (FLEx, WeSay)"],
  ["toolbox", "Toolbox (.txt)"],
];

// Kept across redraws, so a choice survives a text being added elsewhere.
let textFormat: TextFormat = "elan";
let lexiconFormat: LexiconFormat = "lift";
let busy: string | null = null;
// Every open copy of the panel, so the one showing when work ends redraws,
// even if the panel was closed and reopened while it ran.
const live = new Set<() => void>();
const redrawAll = () => {
  for (const draw of live) draw();
};

async function chooseFolder(title: string): Promise<string | null> {
  const folder = await pickFile({ directory: true, title });
  return typeof folder === "string" ? folder : null;
}

/** Run one import or export at a time, showing what it is doing on its button. */
async function work(label: string, action: () => Promise<void>) {
  if (busy !== null) return;
  busy = label;
  redrawAll();
  try {
    await action();
  } catch (e) {
    showToast({ tone: "error", lines: [errorText(e)] });
  }
  busy = null;
  redrawAll();
}

async function importOne() {
  const path = await pickFile({
    title: "Import a file from ELAN, FLEx, Praat or Toolbox",
    filters: [
      { name: "ELAN, FLEx, Praat, Toolbox, plain text", extensions: ["eaf", "flextext", "lift", "TextGrid", "textgrid", "db", "sfm", "lex", "txt"] },
      { name: "All files", extensions: ["*"] },
    ],
  });
  if (typeof path !== "string") return;
  await work("import", async () => {
    const r = await importFile({ path });
    // The report first: a failed refresh below must not hide that the import worked.
    showToast({ tone: "info", lines: [`Imported ${r.format}.`, ...r.lines] });
    if (r.lexicon) touchLexicon();
    await attempt(async () => {
      await reloadTexts();
      if (r.text !== null) await selectText(r.text);
    });
  });
}

async function exportOne(textId: string) {
  const folder = await chooseFolder("Choose where the exported text goes");
  if (folder === null) return;
  await work("text", async () => {
    const r = await exportText({ textId, format: textFormat, folder });
    showToast({ tone: r.notes.length > 0 ? "warn" : "info", lines: [`Saved ${r.path}.`, ...r.notes] });
  });
}

async function exportLex() {
  const folder = await chooseFolder("Choose where the exported lexicon goes");
  if (folder === null) return;
  await work("lexicon", async () => {
    const r = await exportLexicon({ format: lexiconFormat, folder });
    showToast({ tone: "info", lines: [`Saved ${r.path}.`, ...r.notes] });
  });
}

async function seal() {
  const folder = await chooseFolder("Choose where the archive package goes");
  if (folder === null) return;
  await work("seal", async () => {
    const r = await sealProject({ folder, date: localDay(new Date()) });
    showToast({
      tone: r.restricted ? "warn" : "info",
      lines: [
        `Sealed ${r.texts} text${r.texts === 1 ? "" : "s"} and ${r.lexemes} lexicon entr${r.lexemes === 1 ? "y" : "ies"} into ${r.path} (${r.files} files, ${megabytes(r.bytes)}).`,
        "It holds ELAN, FLEx and LIFT copies, archive metadata, every recording and the full history, with checksums (BagIt), ready to deposit.",
        ...(r.restricted ? ["Some of the project is restricted, and the package holds it too: treat the package as restricted."] : []),
      ],
    });
  });
}

function select<T extends string>(options: [T, string][], value: T, onchange: (v: T) => void): HTMLSelectElement {
  const el = h("select", {}, ...options.map(([id, label]) => h("option", { value: id, textContent: label, selected: id === value })));
  el.onchange = () => onchange(el.value as T);
  return el;
}

export const textsPanel: PanelDef = {
  title: "Texts",
  create() {
    const el = h("div", { className: "panel panel-texts" });
    const slice = () => {
      const s = getState();
      return s.kind === "open" ? `${s.info.projectId}|${s.text?.id ?? ""}|${JSON.stringify(s.texts)}` : "closed";
    };
    const draw = () => {
      const s = getState();
      if (s.kind === "closed") {
        el.replaceChildren(emptyState("No project open", "Open or create a project from the top bar."));
        return;
      }
      const title = h("input", { placeholder: "New text title" });
      const form = h(
        "form",
        {
          className: "new-text",
          onsubmit: (ev: SubmitEvent) => {
            ev.preventDefault();
            void newText(title.value.trim());
          },
        },
        title,
        h("button", { type: "submit", textContent: "New text" }),
      );
      const list =
        s.texts.length === 0
          ? h("p", { className: "empty", textContent: "No texts yet. Make one above, or import a file from ELAN, FLEx, Praat or Toolbox below." })
          : h(
              "ul",
              { className: "text-list" },
              ...s.texts.map((t) =>
                h(
                  "li",
                  { className: t.id === s.text?.id ? "selected" : "", onclick: () => void selectText(t.id) },
                  h("span", { textContent: t.title || "(untitled)" }),
                  h("span", { className: "count", textContent: String(t.segments) }),
                ),
              ),
            );
      const text = s.text;
      const share = h(
        "section",
        { className: "share" },
        h("h3", { textContent: "Import and export" }),
        h(
          "div",
          { className: "share-row" },
          h("button", {
            type: "button",
            textContent: busy === "import" ? "Importing…" : "Import a file…",
            title: "ELAN .eaf, FLEx .flextext or .lift, Praat .TextGrid, Toolbox .db/.sfm, or plain text .txt (a segment per sentence)",
            disabled: busy !== null,
            onclick: () => void importOne(),
          }),
        ),
        h(
          "div",
          { className: "share-row" },
          h("span", { className: "share-label", textContent: text === null ? "Select a text to export it" : `«${text.title || "untitled"}» as` }),
          text === null ? null : select(TEXT_FORMATS, textFormat, (v) => (textFormat = v)),
          text === null
            ? null
            : h("button", {
                type: "button",
                textContent: busy === "text" ? "Exporting…" : "Export…",
                disabled: busy !== null,
                onclick: () => void exportOne(text.id),
              }),
        ),
        h(
          "div",
          { className: "share-row" },
          h("span", { className: "share-label", textContent: "Lexicon as" }),
          select(LEXICON_FORMATS, lexiconFormat, (v) => (lexiconFormat = v)),
          h("button", {
            type: "button",
            textContent: busy === "lexicon" ? "Exporting…" : "Export…",
            disabled: busy !== null,
            onclick: () => void exportLex(),
          }),
        ),
        h(
          "div",
          { className: "share-row" },
          h("button", {
            type: "button",
            textContent: busy === "seal" ? "Sealing…" : "Seal a version for the archive…",
            title: "A self-describing package (BagIt) with every text, the lexicon, recordings and full history, for ELAR, PARADISEC or AILLA",
            disabled: busy !== null,
            onclick: () => void seal(),
          }),
        ),
      );
      el.replaceChildren(form, list, share);
    };
    const view = sliced(el, slice, draw);
    live.add(draw);
    return {
      ...view,
      dispose: () => {
        live.delete(draw);
        view.dispose?.();
      },
    };
  },
};
