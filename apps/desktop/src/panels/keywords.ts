// Keywords (Scrolls, SCROLLS.md §5 step 6): the words one text uses more,
// or less, than the project's other texts, scored by log-likelihood and log
// ratio as AntConc's Keyword List and Sketch Engine's keywords do. A row
// opens the Concordance on that word in the text.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, keywords } from "../api";
import { emptyState, h } from "../dom";
import { asTsv, KEYWORD_SORTS, keywordRows, perThousand, significant, summary, type KeywordSide, type KeywordSort, type Keywords } from "../keywords/logic";
import { provide } from "../palette/registry";
import { dataGeneration, getState, showToast } from "../store";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

type Kind = "word" | "translation";
const MINIMUMS = [1, 2, 3, 5, 10];
const DRAWN = 500;

export const keywordsPanel: PanelDef = {
  title: "Keywords",
  create(): PanelView {
    const el = h("div", { className: "panel panel-keywords" });
    let kind: Kind = "word";
    let side: KeywordSide = "more";
    let sortBy: KeywordSort = "ll";
    let textId = "";
    let data: Keywords | null = null;
    let seq = 0;
    const textPicker = h("select", { title: "The text to compare with the project's other texts" });
    textPicker.setAttribute("aria-label", "Text");
    const min = h("select", { title: "Leave out rare words: one used more here needs this many uses here, one used less needs this many in the other texts" }, ...MINIMUMS.map((n) => h("option", { value: String(n), textContent: n === 1 ? "Every word" : `Seen ${n}+ times` })));
    min.setAttribute("aria-label", "Minimum");
    min.value = "2";
    const sorter = h("select", { title: "Order of the rows" }, ...KEYWORD_SORTS.map((x) => h("option", { value: x.id, textContent: x.label })));
    sorter.setAttribute("aria-label", "Sort");
    const kinds = h("div", { className: "scopes", role: "radiogroup", ariaLabel: "Compare" });
    const sides = h("div", { className: "scopes", role: "radiogroup", ariaLabel: "Show" });
    const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy these rows as a table to paste into a spreadsheet" });
    const note = h("p", { className: "note fq-count" });
    const table = h("table", { className: "fq-table kw-table" });

    const chips = <T extends string>(box: HTMLElement, options: { id: T; label: string; title: string }[], current: () => T, pick: (v: T) => void) =>
      box.replaceChildren(
        ...options.map((o) => {
          const b = h("button", { type: "button", textContent: o.label, title: o.title });
          b.setAttribute("role", "radio");
          b.setAttribute("aria-checked", String(o.id === current()));
          b.onclick = () => pick(o.id);
          return b;
        }),
      );
    const drawChips = () => {
      chips(kinds, [{ id: "word" as Kind, label: "Words", title: "Compare transcribed words" }, { id: "translation" as Kind, label: "Translations", title: "Compare the words of free translations" }], () => kind, (v) => {
        kind = v;
        drawChips();
        void load();
      });
      chips(sides, [{ id: "more" as KeywordSide, label: "Used more", title: "Words this text uses more than the other texts" }, { id: "less" as KeywordSide, label: "Used less", title: "Words this text uses less than the other texts" }], () => side, (v) => {
        side = v;
        drawChips();
        drawRows();
      });
    };

    const drawTexts = () => {
      const s = getState();
      const texts = s.kind === "open" ? s.texts : [];
      if (!texts.some((t) => t.id === textId)) textId = (s.kind === "open" && s.text !== null ? s.text.id : texts[0]?.id) ?? "";
      textPicker.replaceChildren(...texts.map((t) => h("option", { value: t.id, textContent: t.title || t.id })));
      textPicker.value = textId;
    };
    const title = () => textPicker.selectedOptions[0]?.textContent ?? "";
    const shown = () => (data === null ? [] : keywordRows(data.rows, side, sortBy));

    const drawRows = () => {
      if (data === null) {
        table.replaceChildren();
        return;
      }
      const rows = shown();
      note.textContent = summary(title(), data) + (rows.length === 0 && data.referenceTokens > 0 ? ` No word is used ${side === "more" ? "more" : "less"} here.` : "") + (rows.length > DRAWN ? ` The first ${DRAWN} are drawn.` : "");
      const head = h(
        "tr",
        {},
        ...[
          ["Word", ""],
          ["Here", "Times in this text (per 1000 words on hover)"],
          ["Rest", "Times in the project's other texts (per 1000 words on hover)"],
          ["LL", "Log-likelihood (4-term, as AntConc): how sure the difference is; below zero when the text uses the word less"],
          ["Ratio", "Log ratio: 1 means twice as often here, −1 half as often (an absent word counts as 0.5)"],
        ].map(([t, tip]) => h("th", { textContent: t, scope: "col", title: tip })),
      );
      const text = { id: textId, title: title() };
      table.replaceChildren(
        h("thead", {}, head),
        h(
          "tbody",
          {},
          ...rows.slice(0, DRAWN).map((r) => {
            const tr = h(
              "tr",
              { tabIndex: 0, title: `${perThousand(r.focus, data!.focusTokens)} per 1000 here, ${perThousand(r.reference, data!.referenceTokens)} in the rest; %DIFF ${r.percentDiff.toFixed(0)}; simple maths ${r.simpleMaths.toFixed(2)}. Show every «${r.item}» ${r.focus > 0 ? "in this text" : "in the whole project"} in the Concordance` },
              h("td", { className: "fq-item", textContent: (significant(r, data!.critical) ? "• " : "") + r.item }),
              h("td", { className: "fq-num", textContent: r.focus.toLocaleString("en") }),
              h("td", { className: "fq-num", textContent: r.reference.toLocaleString("en") }),
              h("td", { className: "fq-num", textContent: r.logLikelihood.toFixed(2) }),
              h("td", { className: "fq-num", textContent: r.logRatio.toFixed(2) }),
            );
            const open = () => askConcordance(r.item, kind, r.focus > 0 ? text : null);
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
      data = null;
      table.replaceChildren();
      if (textId === "") {
        note.textContent = "Add or import texts to compare them.";
        return;
      }
      note.textContent = "Counting…";
      try {
        const r = await keywords({ kind, textId, minCount: Number(min.value) });
        if (mine !== seq) return;
        data = r;
        drawRows();
      } catch (e) {
        if (mine !== seq) return;
        note.textContent = `Could not count: ${errorText(e)}`;
      }
    }

    textPicker.onchange = () => {
      textId = textPicker.value;
      void load();
    };
    min.onchange = () => void load();
    sorter.onchange = () => {
      sortBy = sorter.value as KeywordSort;
      drawRows();
    };
    copy.onclick = () => {
      const rows = shown();
      if (rows.length === 0) return;
      void writeText(asTsv(rows)).then(
        () => showToast({ tone: "info", lines: [`Copied ${rows.length} rows. Paste them into a spreadsheet.`] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };

    const unprovide = provide("keywords", () => [
      { id: "keywords:text", title: "Find the keywords of a text", group: "Keywords", words: ["keyness", "key words", "log ratio", "antconc", "compare texts"], run: () => textPicker.focus() },
    ]);

    let drawnFor = "";
    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}|${s.texts.map((t) => `${t.id}:${t.title}:${t.segments}`).join(",")}` : "closed";
      if (key === drawnFor) return;
      drawnFor = key;
      if (s.kind !== "open") {
        seq += 1;
        data = null;
        el.replaceChildren(emptyState("Keywords", "Open a project to see which words set each text apart.", "The wug is weighing every word."));
        return;
      }
      drawChips();
      drawTexts();
      el.replaceChildren(h("div", { className: "cc-bar" }, textPicker), kinds, sides, h("div", { className: "cc-bar" }, min, sorter, copy), note, h("div", { className: "fq-rows" }, table));
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
