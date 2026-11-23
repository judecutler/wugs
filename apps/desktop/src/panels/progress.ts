// Progress: how far the documentation has come, text by text, for a grant
// report, a deposit form or a supervisor (src-tauri/src/progress.rs). Words
// and sums in ../progress/logic.ts. Read-only; a row opens its text.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, projectProgress } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { COLUMNS, progressTsv, rowCells, summaryLines, totals, type Progress } from "../progress/logic";
import { dataGeneration, getState, selectText, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";

export const progressPanel: PanelDef = {
  title: "Progress",
  create(): PanelView {
    const el = h("div", { className: "panel panel-progress" });
    let data: Progress | null = null;
    let seq = 0;
    let shown = "";
    const summary = h("ul", { className: "pg-summary" });
    const table = h("table", { className: "pg-table" });
    const note = h("p", { className: "note" });
    const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy every text's figures and the totals, to paste into a spreadsheet or a report" });
    const recount = h("button", { type: "button", textContent: "Count again" });

    const draw = () => {
      if (data === null) {
        summary.replaceChildren();
        table.replaceChildren();
        return;
      }
      summary.replaceChildren(...summaryLines(data).map((line) => h("li", { textContent: line })));
      const row = (cells: string[], id: string | null) => {
        const tr = h("tr", {}, ...cells.map((c, i) => h(i === 0 ? "th" : "td", i === 0 ? { scope: "row", textContent: c } : { textContent: c })));
        if (id !== null) {
          tr.tabIndex = 0;
          tr.title = "Open this text";
          tr.onclick = () => void selectText(id);
          tr.onkeydown = (e) => {
            if (e.key === "Enter") void selectText(id);
          };
        }
        return tr;
      };
      const parts: Node[] = [h("thead", {}, h("tr", {}, ...COLUMNS.map((c) => h("th", { scope: "col", textContent: c })))), h("tbody", {}, ...data.texts.map((t) => row(rowCells(t), t.id)))];
      if (data.texts.length > 1) parts.push(h("tfoot", {}, row(rowCells(totals(data)), null)));
      table.replaceChildren(...parts);
      note.textContent =
        "A sentence is a segment on a transcription track; a word is glossed once it is broken into morphs; speech transcribed adds up the timed sentences that have a transcription.";
    };

    async function load() {
      const mine = ++seq;
      try {
        const r = await projectProgress();
        if (mine !== seq) return;
        data = r;
      } catch (e) {
        if (mine !== seq) return;
        data = null;
        note.textContent = `Could not count: ${errorText(e)}`;
      }
      draw();
    }

    copy.onclick = () => {
      if (data === null) return;
      void writeText(progressTsv(data)).then(
        () => showToast({ tone: "info", lines: ["Copied the progress table. Paste it into a spreadsheet or a report."] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };
    recount.onclick = () => void load();

    const unprovide = provide("progress", () => [
      { id: "progress:copy", title: "Copy the progress table", group: "Progress", words: ["report", "grant", "statistics", "hours", "how much"], run: () => copy.click() },
    ]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}|${s.texts.map((t) => `${t.id}:${t.segments}`).join(",")}` : "closed";
      if (key === shown) return;
      shown = key;
      if (s.kind !== "open") {
        seq += 1;
        data = null;
        el.replaceChildren(emptyState("Progress", "Open a project to see how far it has come.", "The wug is keeping score."));
        return;
      }
      el.replaceChildren(h("div", { className: "cc-bar" }, recount, copy), summary, h("div", { className: "pg-rows" }, table), note);
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
