// Frequencies (Scrolls, SCROLLS.md §5 step 3): how often each word, morph,
// gloss or translation word occurs, in the whole project or one text. A row
// opens the Concordance on that item, so every count can be checked line by line.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, frequencies } from "../api";
import { KINDS, type ConcordanceKind } from "../concordance/logic";
import { emptyState, h } from "../dom";
import { asTsv, filterRows, FREQ_SORTS, perThousand, sortRows, summary, type FreqSort, type Frequencies } from "../frequencies/logic";
import { provide } from "../palette/registry";
import { dataGeneration, getState, showToast } from "../store";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

const NOUNS: Record<ConcordanceKind, { one: string; many: string }> = {
  word: { one: "word", many: "words" },
  morph: { one: "morph", many: "morphs" },
  gloss: { one: "gloss", many: "glosses" },
  translation: { one: "translation word", many: "translation words" },
};

/** Rows drawn at once; the filter finds the rest. */
const DRAWN = 500;

export const frequenciesPanel: PanelDef = {
  title: "Frequencies",
  create(): PanelView {
    const el = h("div", { className: "panel panel-frequencies" });
    let kind: ConcordanceKind = "word";
    let sortBy: FreqSort = "count";
    let scope = "";
    let data: Frequencies | null = null;
    let seq = 0;
    const chips = h("div", { className: "scopes", role: "radiogroup", ariaLabel: "Count" });
    const scopePicker = h("select", { className: "fq-scope", title: "Count in the whole project or one text" });
    scopePicker.setAttribute("aria-label", "Where to count");
    const sorter = h("select", { className: "fq-sort", title: "Order of the rows" }, ...FREQ_SORTS.map((x) => h("option", { value: x.id, textContent: x.label })));
    sorter.setAttribute("aria-label", "Sort");
    const filter = h("input", { type: "search", className: "fq-filter", placeholder: "Filter the list", spellcheck: false });
    filter.setAttribute("aria-label", "Filter");
    const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy the list as a table to paste into a spreadsheet" });
    const count = h("p", { className: "note fq-count" });
    const table = h("table", { className: "fq-table" });

    const drawChips = () =>
      chips.replaceChildren(
        ...KINDS.map((k) => {
          const b = h("button", { type: "button", textContent: k.label, title: `Count ${NOUNS[k.id].many}` });
          b.setAttribute("role", "radio");
          b.setAttribute("aria-checked", String(k.id === kind));
          b.onclick = () => {
            kind = k.id;
            drawChips();
            void load();
          };
          return b;
        }),
      );

    const drawScopes = () => {
      const s = getState();
      const texts = s.kind === "open" ? s.texts : [];
      if (scope !== "" && !texts.some((t) => t.id === scope)) scope = "";
      scopePicker.replaceChildren(h("option", { value: "", textContent: "Whole project" }), ...texts.map((t) => h("option", { value: t.id, textContent: t.title || t.id })));
      scopePicker.value = scope;
    };

    const shownRows = () => (data === null ? [] : sortRows(filterRows(data.rows, filter.value), sortBy));

    const drawRows = () => {
      if (data === null) {
        table.replaceChildren();
        return;
      }
      const rows = shownRows();
      const tokens = data.tokens;
      count.textContent = summary(data, NOUNS[kind]) + (filter.value.trim() !== "" ? ` ${rows.length} match the filter.` : "");
      const head = h("tr", {}, ...["#", NOUNS[kind].one[0]!.toUpperCase() + NOUNS[kind].one.slice(1), "Count", "Per 1000", "Texts"].map((t) => h("th", { textContent: t, scope: "col" })));
      table.replaceChildren(
        h("thead", {}, head),
        h(
          "tbody",
          {},
          ...rows.slice(0, DRAWN).map((r, i) => {
            const tr = h(
              "tr",
              { tabIndex: 0, title: `Show every «${r.item}» in the Concordance` },
              h("td", { className: "fq-rank", textContent: String(i + 1) }),
              h("td", { className: "fq-item", textContent: r.item }),
              h("td", { className: "fq-num", textContent: r.count.toLocaleString("en") }),
              h("td", { className: "fq-num", textContent: perThousand(r.count, tokens) }),
              h("td", { className: "fq-num", textContent: String(r.texts) }),
            );
            const open = () => askConcordance(r.item, kind);
            tr.onclick = open;
            tr.onkeydown = (e) => {
              if (e.key === "Enter") open();
              else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                e.preventDefault();
                const next = e.key === "ArrowDown" ? tr.nextElementSibling : tr.previousElementSibling;
                if (next instanceof HTMLElement) next.focus();
              }
            };
            return tr;
          }),
        ),
      );
    };

    async function load(): Promise<void> {
      const mine = ++seq;
      count.textContent = "Counting…";
      try {
        const r = await frequencies({ kind, textId: scope === "" ? null : scope });
        if (mine !== seq) return;
        data = r;
        drawRows();
      } catch (e) {
        if (mine !== seq) return;
        data = null;
        drawRows();
        count.textContent = `Could not count: ${errorText(e)}`;
      }
    }

    scopePicker.onchange = () => {
      scope = scopePicker.value;
      void load();
    };
    sorter.onchange = () => {
      sortBy = sorter.value as FreqSort;
      drawRows();
    };
    filter.oninput = drawRows;
    copy.onclick = () => {
      if (data === null || data.rows.length === 0) return;
      const rows = shownRows();
      void writeText(asTsv(rows, data.tokens)).then(
        () => showToast({ tone: "info", lines: [`Copied ${rows.length} rows. Paste them into a spreadsheet.`] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };

    const unprovide = provide("frequencies", () => [
      { id: "frequencies:filter", title: "Filter the frequency list", group: "Frequencies", words: ["word list", "counts", "corpus", "types", "tokens"], run: () => filter.focus() },
    ]);

    let shown = "";
    const render = () => {
      const s = getState();
      const project = s.kind === "open" ? s.info.projectId : "closed";
      const key = s.kind === "open" ? `${project}|${dataGeneration()}|${s.texts.map((t) => `${t.id}:${t.title}:${t.segments}`).join(",")}` : "closed";
      if (key === shown) return;
      const sameProject = shown.split("|")[0] === project;
      shown = key;
      if (s.kind !== "open") {
        seq += 1;
        data = null;
        el.replaceChildren(emptyState("Frequencies", "Open a project to count its words, morphs and glosses.", "The wug is counting on its toes."));
        return;
      }
      if (!sameProject) {
        scope = "";
        filter.value = "";
        data = null;
      }
      drawChips();
      drawScopes();
      el.replaceChildren(h("div", { className: "cc-bar" }, scopePicker, sorter, copy), chips, h("div", { className: "cc-bar" }, filter), count, h("div", { className: "fq-rows" }, table));
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
