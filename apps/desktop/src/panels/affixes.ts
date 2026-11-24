// Affixes: the project's affixes and clitics read from its glossed words
// (src-tauri/src/affixes.rs), as the position-class chart a grammar draws
// and a table behind it. Words and tables in ../affixes/logic.ts. Read-only:
// an affix opens the Concordance on it, an example opens its sentence.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, projectAffixes } from "../api";
import { affixTsv, chartTsv, elsewhere, marked, pairLine, pairNote, pairs, pairsSummary, pairsTsv, slotChart, slotLabel, summaryLine, type Affix, type Affixes } from "../affixes/logic";
import { emptyState, h } from "../dom";
import { glossRuns } from "../gloss/example";
import { provide } from "../palette/registry";
import { dataGeneration, getState, selectSegment, showToast } from "../store";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

const gloss = (g: string) => h("span", { className: "af-gloss" }, ...glossRuns(g).map((r) => (r.caps ? h("span", { className: "sc", textContent: r.text }) : document.createTextNode(r.text))));

export const affixesPanel: PanelDef = {
  title: "Affixes",
  create(): PanelView {
    const el = h("div", { className: "panel panel-affixes" });
    let data: Affixes | null = null;
    let seq = 0;
    let shown = "";
    const summary = h("p", { className: "note" });
    const chart = h("div", { className: "af-chart" });
    const table = h("table", { className: "pg-table af-table" });
    const copyChart = h("button", { type: "button", textContent: "Copy chart", title: "The position-class chart as a table, to paste into a grammar or a spreadsheet" });
    const copyTable = h("button", { type: "button", textContent: "Copy table", title: "Every affix with its gloss, slot, uses, roots and an example" });
    const recount = h("button", { type: "button", textContent: "Read again" });
    const orderNote = h("p", { className: "note" });
    const orderTable = h("table", { className: "pg-table af-order" });
    const copyOrder = h("button", { type: "button", textContent: "Copy pairs", title: "Every two affixes met in one word, with how often each order is met" });

    const find = (a: Affix) => askConcordance(a.surface, "morph", null, true);

    const chip = (a: Affix) => {
      const b = h("button", { type: "button", className: "af-chip", title: `${a.uses} ${a.uses === 1 ? "use" : "uses"} on ${a.roots} ${a.roots === 1 ? "root" : "roots"}. Every word with it in the Concordance` });
      b.append(h("strong", { textContent: marked(a) }), " ", gloss(a.gloss), h("span", { className: "af-n", textContent: ` ×${a.uses}` }));
      b.onclick = () => find(a);
      return b;
    };

    const draw = () => {
      if (data === null) {
        chart.replaceChildren();
        table.replaceChildren();
        orderTable.replaceChildren();
        orderNote.textContent = "";
        return;
      }
      const ps = pairs(data);
      orderNote.textContent = pairsSummary(ps);
      copyOrder.disabled = ps.length === 0;
      orderTable.replaceChildren(
        ...(ps.length === 0
          ? []
          : [
              h("thead", {}, h("tr", {}, ...["Together", "Words"].map((t) => h("th", { scope: "col", textContent: t })))),
              h(
                "tbody",
                {},
                ...ps.map((p) => {
                  const note = pairNote(p);
                  return h(
                    "tr",
                    { className: note === "" ? "" : "af-flag" },
                    h(
                      "th",
                      { scope: "row" },
                      h("span", { textContent: pairLine(p) }),
                      " ",
                      gloss(`${p.a.gloss} ${p.b.gloss}`.trim()),
                      ...(note === "" ? [] : [h("div", { className: "af-note", textContent: note })]),
                    ),
                    h("td", { textContent: String(p.ab + p.ba) }),
                  );
                }),
              ),
            ]),
      );
      summary.textContent = summaryLine(data);
      const columns = slotChart(data.affixes);
      chart.replaceChildren(
        ...(data.affixes.length === 0
          ? []
          : columns.map((c) =>
              h(
                "div",
                { className: c.slot === 0 ? "af-col af-root" : "af-col" },
                h("div", { className: "af-head", textContent: slotLabel(c.slot) }),
                ...(c.slot === 0 ? [h("div", { className: "af-rootbox", textContent: "ROOT" })] : c.affixes.map(chip)),
              ),
            )),
      );
      const head = ["Affix", "Gloss", "Slot", "Uses", "Roots", "Root categories", "Example"];
      table.replaceChildren(
        h("thead", {}, h("tr", {}, ...head.map((t) => h("th", { scope: "col", textContent: t })))),
        h(
          "tbody",
          {},
          ...data.affixes.map((a) => {
            const where = elsewhere(a);
            const example = h("button", { type: "button", className: "linkish", textContent: a.example.word, title: "Open the sentence this word is in" });
            example.onclick = () => selectSegment({ textId: a.example.textId, tierId: a.example.tierId, segmentId: a.example.segmentId });
            return h(
              "tr",
              {},
              h("th", { scope: "row", textContent: marked(a), title: a.kind }),
              h("td", {}, gloss(a.gloss)),
              h("td", { textContent: where ? `${slotLabel(a.slot)} (${where})` : slotLabel(a.slot) }),
              h("td", { textContent: String(a.uses) }),
              h("td", { textContent: String(a.roots) }),
              h("td", { textContent: a.categories.map(([c, n]) => `${c} ${n}`).join(", ") }),
              h("td", {}, example),
            );
          }),
        ),
      );
    };

    async function load() {
      const mine = ++seq;
      try {
        const r = await projectAffixes();
        if (mine !== seq) return;
        data = r;
      } catch (e) {
        if (mine !== seq) return;
        data = null;
        summary.textContent = `Could not read the affixes: ${errorText(e)}`;
      }
      draw();
    }

    const copy = (what: string, text: () => string | null) => {
      const t = text();
      if (t === null) return;
      void writeText(t).then(
        () => showToast({ tone: "info", lines: [`Copied the ${what}. Paste it into a spreadsheet or a document.`] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };
    copyChart.onclick = () => copy("position-class chart", () => (data === null ? null : chartTsv(slotChart(data.affixes))));
    copyTable.onclick = () => copy("affix table", () => (data === null ? null : affixTsv(data.affixes)));
    copyOrder.onclick = () => copy("pairs of affixes", () => (data === null ? null : pairsTsv(pairs(data))));
    recount.onclick = () => void load();

    const unprovide = provide("affixes", () => [
      { id: "affixes:chart", title: "Copy the position-class chart", group: "Affixes", words: ["slot", "template", "morphology", "affix order"], run: () => copyChart.click() },
    ]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}` : "closed";
      if (key === shown) return;
      shown = key;
      if (s.kind !== "open") {
        seq += 1;
        data = null;
        el.replaceChildren(emptyState("Affixes", "Open a project to see its affixes in their slots.", "The wug is sorting the pieces."));
        return;
      }
      el.replaceChildren(
        h("div", { className: "cc-bar" }, recount, copyChart, copyTable),
        summary,
        chart,
        h("p", { className: "note", textContent: "Each affix stands in the slot it holds most often, counted from the root: −1 right before it, +1 right after. Click one for every word it is in." }),
        h("div", { className: "pg-rows" }, table),
        h("h3", { textContent: "Order" }),
        h("div", { className: "cc-bar" }, copyOrder),
        orderNote,
        h("div", { className: "pg-rows" }, orderTable),
        h("p", { className: "note", textContent: "Two affixes in one slot should never occur in one word, and two in different slots should always come in the slots' order. Rows that break either are marked." }),
      );
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
