// Progress: how far the documentation has come, text by text, for a grant
// report, a deposit form or a supervisor (src-tauri/src/progress.rs). Words
// and sums in ../progress/logic.ts. Read-only; a row opens its text.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, projectProgress, vocabularyGrowth } from "../api";
import { growthNote, growthPath, ticks, type Box, type Growth } from "../progress/growth";
import { composition, compositionNote, compositionTsv, detailKeys, keyLabel, wordShare } from "../progress/composition";
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
    let growth: Growth | null = null;
    let detail = "";
    const makeUp = h("div", { className: "pg-makeup" });
    const growthBox = h("div", { className: "pg-growth" });
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
      drawMakeUp();
      note.textContent =
        "A sentence is a segment on a transcription track; a word is glossed once it is broken into morphs; speech transcribed adds up the timed sentences that have a transcription.";
    };

    const NS = "http://www.w3.org/2000/svg";
    const svg = (tag: string, attrs: Record<string, string | number>, text?: string): SVGElement => {
      const e = document.createElementNS(NS, tag);
      for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
      if (text !== undefined) e.textContent = text;
      return e;
    };

    const drawGrowth = () => {
      if (growth === null) {
        growthBox.replaceChildren();
        return;
      }
      const g = growth;
      const parts: Node[] = [h("h3", { className: "pg-head", textContent: "Vocabulary growth" })];
      if (g.tokens > 0) {
        const box: Box = { width: 420, height: 204, left: 46, right: 10, top: 10, bottom: 34 };
        const w = box.width - box.left - box.right;
        const ht = box.height - box.top - box.bottom;
        const chart = svg("svg", { viewBox: `0 0 ${box.width} ${box.height}`, class: "pg-chart", role: "img" });
        chart.setAttribute("aria-label", "Different words met against running words, texts in the order they were made");
        for (const t of ticks(g.tokens)) {
          const x = box.left + (t * w) / g.tokens;
          chart.append(svg("line", { x1: x, x2: x, y1: box.top, y2: box.top + ht, class: "grid" }), svg("text", { x, y: box.height - 19, class: "tick", "text-anchor": "middle" }, t.toLocaleString("en")));
        }
        for (const t of ticks(g.types)) {
          const y = box.top + ht - (g.types === 0 ? 0 : (t * ht) / g.types);
          chart.append(svg("line", { x1: box.left, x2: box.left + w, y1: y, y2: y, class: "grid" }), svg("text", { x: box.left - 6, y: y + 4, class: "tick", "text-anchor": "end" }, t.toLocaleString("en")));
        }
        for (const t of g.texts.slice(1)) {
          const x = box.left + (t.at * w) / g.tokens;
          const mark = svg("line", { x1: x, x2: x, y1: box.top, y2: box.top + ht, class: "text-start" });
          mark.append(svg("title", {}, `«${t.title}» starts here`));
          chart.append(mark);
        }
        chart.append(svg("path", { d: growthPath(g, box), class: "curve" }));
        chart.append(svg("text", { x: box.left + w / 2, y: box.height - 3, class: "axis", "text-anchor": "middle" }, "running words"));
        chart.append(svg("text", { x: 0, y: 0, class: "axis", "text-anchor": "middle", transform: `translate(10 ${box.top + ht / 2}) rotate(-90)` }, "different words"));
        parts.push(chart);
      }
      parts.push(h("p", { className: "note", textContent: `${growthNote(g)}${g.texts.length > 1 ? " Dashed lines mark where each text starts, in the order the texts were made." : ""}` }));
      growthBox.replaceChildren(...parts);
    };

    const drawMakeUp = () => {
      if (data === null) {
        makeUp.replaceChildren();
        return;
      }
      const keys = detailKeys(data.texts);
      if (keys.length === 0) {
        makeUp.replaceChildren(
          h("h3", { className: "pg-head", textContent: "The corpus by detail" }),
          h("p", { className: "note", textContent: "Give the texts details (genre, speaker, date…) in the Texts panel's Details or with Import details…, and their share of the words shows here." }),
        );
        return;
      }
      if (!keys.some((k) => k.key === detail)) detail = keys[0]!.key;
      const picker = h("select", { className: "pg-detail", title: "Which detail to break the corpus down by" }, ...keys.map((k) => h("option", { value: k.key, textContent: `${keyLabel(k.key)} (${k.texts} ${k.texts === 1 ? "text" : "texts"})`, selected: k.key === detail })));
      picker.setAttribute("aria-label", "Detail");
      picker.onchange = () => {
        detail = picker.value;
        drawMakeUp();
      };
      const rows = composition(data.texts, detail);
      const copyMakeUp = h("button", { type: "button", textContent: "Copy table", title: "Copy the corpus's make-up by this detail" });
      copyMakeUp.onclick = () =>
        void writeText(compositionTsv(rows, detail)).then(
          () => showToast({ tone: "info", lines: [`Copied the corpus by ${keyLabel(detail)}.`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
      const n = (x: number) => x.toLocaleString("en");
      const head = h("tr", {}, ...[keyLabel(detail), "Texts", "Sentences", "Words", "Share", "Glossed"].map((c) => h("th", { scope: "col", textContent: c[0]!.toUpperCase() + c.slice(1) })));
      const body = rows.map((r) => h("tr", {}, h("th", { scope: "row", textContent: r.value }), h("td", { textContent: n(r.texts) }), h("td", { textContent: n(r.sentences) }), h("td", { textContent: n(r.words) }), h("td", { textContent: wordShare(r, rows) }), h("td", { textContent: n(r.glossed) })));
      makeUp.replaceChildren(
        h("h3", { className: "pg-head", textContent: "The corpus by detail" }),
        h("div", { className: "cc-bar" }, picker, copyMakeUp),
        h("div", { className: "pg-rows" }, h("table", { className: "pg-table" }, h("thead", {}, head), h("tbody", {}, ...body))),
        h("p", { className: "note", textContent: compositionNote(rows, detail) }),
      );
    };

    async function load() {
      const mine = ++seq;
      vocabularyGrowth().then(
        (g) => {
          if (mine !== seq) return;
          growth = g;
          drawGrowth();
        },
        () => {
          if (mine !== seq) return;
          growth = null;
          drawGrowth();
        },
      );
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
      el.replaceChildren(h("div", { className: "cc-bar" }, recount, copy), summary, h("div", { className: "pg-rows" }, table), note, makeUp, growthBox);
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
