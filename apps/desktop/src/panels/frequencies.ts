// Frequencies (Scrolls, SCROLLS.md §5 step 3): how often each word, morph,
// gloss or translation word occurs, in the whole project or one text. A row
// opens the Concordance on that item, so every count can be checked line by line.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, frequencies } from "../api";
import { KINDS, type ConcordanceKind } from "../concordance/logic";
import { emptyState, h } from "../dom";
import { asTsv, filterRows, FREQ_SORTS, perThousand, ranked, sortRows, summary, type FreqSort, type Frequencies, type RankedRow } from "../frequencies/logic";
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
    let rankedRows: RankedRow[] = [];
    let countedAt = "";
    let seq = 0;
    const chips = h("div", { className: "scopes", role: "radiogroup", ariaLabel: "Count" });
    const scopePicker = h("select", { className: "fq-scope", title: "Count in the whole project or one text" });
    scopePicker.setAttribute("aria-label", "Where to count");
    let size = 1;
    const sizer = h(
      "select",
      { className: "fq-size", title: "Count single items, or runs of words in a row inside a segment (n-grams, as AntConc's N-Grams tool)" },
      ...[1, 2, 3, 4, 5].map((n) => h("option", { value: String(n), textContent: n === 1 ? "Single items" : `${n} words in a row` })),
    );
    sizer.setAttribute("aria-label", "Words in a row");
    const ngrams = () => size > 1 && (kind === "word" || kind === "translation");
    const noun = () => (ngrams() ? { one: `${size}-word sequence`, many: `${size}-word sequences` } : NOUNS[kind]);
    const sorter = h("select", { className: "fq-sort", title: "Order of the rows" }, ...FREQ_SORTS.map((x) => h("option", { value: x.id, textContent: x.label })));
    sorter.setAttribute("aria-label", "Sort");
    const filter = h("input", { type: "search", className: "fq-filter", placeholder: "Filter the list", spellcheck: false });
    filter.setAttribute("aria-label", "Filter");
    const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy the list as a table to paste into a spreadsheet" });
    const recount = h("button", { type: "button", textContent: "Recount", title: "Count again (the list counts when the panel opens, the project syncs or texts change)" });
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
            sizer.disabled = !(kind === "word" || kind === "translation");
            sizer.value = String(ngrams() ? size : 1);
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

    const shownRows = () => sortRows(filterRows(rankedRows, filter.value), sortBy);

    const drawRows = () => {
      if (data === null) {
        table.replaceChildren();
        return;
      }
      const rows = shownRows();
      const tokens = data.tokens;
      const perWord = kind === "morph" || kind === "gloss" ? ` Each counts once per word, as the Concordance shows one line per word.` : "";
      const cut = rows.length > DRAWN ? ` The first ${DRAWN} are drawn; filter to find the others (Copy table has them all).` : "";
      count.textContent = summary(data, noun()) + perWord + (filter.value.trim() !== "" ? ` ${rows.length} match the filter.` : "") + cut + ` Counted at ${countedAt}.`;
      const head = h("tr", {}, ...["#", noun().one[0]!.toUpperCase() + noun().one.slice(1), "Count", "Per 1000", "Texts"].map((t) => h("th", { textContent: t, scope: "col" })));
      table.replaceChildren(
        h("thead", {}, head),
        h(
          "tbody",
          {},
          ...rows.slice(0, DRAWN).map((r) => {
            const tr = h(
              "tr",
              { tabIndex: 0, title: `Show every «${r.item}» in the Concordance` },
              h("td", { className: "fq-rank", textContent: String(r.rank) }),
              h("td", { className: "fq-item", textContent: r.item }),
              h("td", { className: "fq-num", textContent: r.count.toLocaleString("en") }),
              h("td", { className: "fq-num", textContent: perThousand(r.count, tokens) }),
              h("td", { className: "fq-num", textContent: String(r.texts) }),
            );
            const text = scope === "" ? null : { id: scope, title: scopePicker.selectedOptions[0]?.textContent ?? "" };
            // A sequence opens on its first word; sorting by the word after brings the sequence together.
            const first = ngrams() ? r.item.split(" ")[0]! : r.item;
            if (ngrams()) tr.title = `Show every «${first}» in the Concordance; sort by the word after to bring «${r.item}» together`;
            const open = () => askConcordance(first, kind, text);
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
      // No row of an earlier count stays clickable while this one runs.
      data = null;
      rankedRows = [];
      table.replaceChildren();
      try {
        const r = await frequencies({ kind, textId: scope === "" ? null : scope, size: ngrams() ? size : 1 });
        if (mine !== seq) return;
        data = r;
        rankedRows = ranked(r.rows);
        countedAt = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
        drawRows();
      } catch (e) {
        if (mine !== seq) return;
        data = null;
        drawRows();
        count.textContent = `Could not count: ${errorText(e)}`;
      }
    }

    sizer.onchange = () => {
      size = Number(sizer.value);
      void load();
    };
    scopePicker.onchange = () => {
      scope = scopePicker.value;
      void load();
    };
    sorter.onchange = () => {
      sortBy = sorter.value as FreqSort;
      drawRows();
    };
    filter.oninput = drawRows;
    recount.onclick = () => void load();
    copy.onclick = () => {
      if (data === null || rankedRows.length === 0) return;
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
      sizer.disabled = !(kind === "word" || kind === "translation");
      sizer.value = String(ngrams() ? size : 1);
      el.replaceChildren(h("div", { className: "cc-bar" }, scopePicker, sizer, sorter, recount, copy), chips, h("div", { className: "cc-bar" }, filter), count, h("div", { className: "fq-rows" }, table));
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
