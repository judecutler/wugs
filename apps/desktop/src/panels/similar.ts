// Similar words (Scrolls): the words used in the same contexts as a word, as
// Sketch Engine's Thesaurus lists them, read from plain text with no parser.
// A row opens the Concordance on that word. Rules in ../similar/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, similarWords } from "../api";
import { emptyState, h } from "../dom";
import { fillScopes, scopeOf } from "../metadata/scope";
import { provide } from "../palette/registry";
import { barWidth, MINIMUMS, similarityCell, similarNote, similarTsv, WINDOWS, type Similar } from "../similar/logic";
import { dataGeneration, getState, showToast } from "../store";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

type Kind = "word" | "translation";
const KIND_CHIPS: { id: Kind; label: string }[] = [
  { id: "word", label: "Words" },
  { id: "translation", label: "Translations" },
];

export const similarPanel: PanelDef = {
  title: "Similar words",
  create(): PanelView {
    const el = h("div", { className: "panel panel-similar" });
    let kind: Kind = "word";
    let scope = "";
    let data: Similar | null = null;
    let asked = "";
    let seq = 0;
    const word = h("input", { type: "search", className: "co-node", placeholder: "A word", spellcheck: false });
    word.setAttribute("aria-label", "Word to find similar words for");
    const go = h("button", { type: "button", className: "primary", textContent: "Find similar words" });
    const windowPick = h("select", { title: "How many words either side count as a word's context: 1 or 2 finds words of one class, 5 words of one topic" }, ...WINDOWS.map((n) => h("option", { value: String(n), textContent: `${n} either side` })));
    windowPick.setAttribute("aria-label", "Context window");
    windowPick.value = "2";
    const minPick = h("select", { title: "Compare only words met at least this many times" }, ...MINIMUMS.map((n) => h("option", { value: String(n), textContent: `Words met ${n}+ times` })));
    minPick.setAttribute("aria-label", "Minimum frequency");
    minPick.value = "3";
    const chips = h("div", { className: "scopes", role: "radiogroup", ariaLabel: "Look among" });
    const scopePicker = h("select", { title: "Look in the whole project, one text, or the texts with one detail" });
    scopePicker.setAttribute("aria-label", "Where to look");
    const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy the list as a table to paste into a spreadsheet" });
    const note = h("p", { className: "note fq-count" });
    const table = h("table", { className: "fq-table sm-table" });

    const drawChips = () =>
      chips.replaceChildren(
        ...KIND_CHIPS.map((k) => {
          const b = h("button", { type: "button", textContent: k.label, title: k.id === "word" ? "Among transcribed words" : "Among the words of free translations" });
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
      scope = fillScopes(scopePicker, scope, drawScopes);
    };

    const drawRows = () => {
      if (data === null) {
        table.replaceChildren();
        return;
      }
      note.textContent = similarNote(data, asked, Number(windowPick.value));
      copy.disabled = data.rows.length === 0;
      if (data.rows.length === 0) {
        table.replaceChildren();
        return;
      }
      const sc = scopeOf(scope);
      const text = sc.textId === null ? null : { id: sc.textId, title: scopePicker.selectedOptions[0]?.textContent ?? "" };
      table.replaceChildren(
        h(
          "thead",
          {},
          h(
            "tr",
            {},
            ...[
              ["Word", ""],
              ["Similarity", "Cosine of the two words' context profiles (positive PMI): 1 when they are used in exactly the same company"],
              ["Freq", "Times it occurs here"],
              ["Shared contexts", "The words around both that weigh most in the similarity"],
            ].map(([t, tip]) => h("th", { scope: "col", textContent: t, title: tip })),
          ),
        ),
        h(
          "tbody",
          {},
          ...data.rows.map((r) => {
            const bar = h("span", { className: "sm-bar" });
            bar.style.width = `${barWidth(r.similarity)}%`;
            const tr = h(
              "tr",
              { tabIndex: 0, title: `Read «${r.item}» in context in the Concordance` },
              h("td", { className: "fq-item", textContent: r.item }),
              h("td", { className: "fq-num sm-score" }, h("span", { className: "sm-track" }, bar), similarityCell(r.similarity)),
              h("td", { className: "fq-num", textContent: r.freq.toLocaleString("en") }),
              h("td", { className: "sm-shared", textContent: r.shared.join(", ") }),
            );
            const open = () => askConcordance(r.item, kind, text, true, sc.within);
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
      const query = word.value.trim();
      const mine = ++seq;
      data = null;
      table.replaceChildren();
      copy.disabled = true;
      if (query === "") {
        note.textContent = "Type a word to see the words used in the same company: candidates for synonyms, antonyms and words of one class.";
        return;
      }
      note.textContent = "Comparing…";
      try {
        const sc = scopeOf(scope);
        const r = await similarWords({ word: query, kind, window: Number(windowPick.value), minFreq: Number(minPick.value), textId: sc.textId, within: sc.within });
        if (mine !== seq) return;
        data = r;
        asked = query;
        drawRows();
      } catch (e) {
        if (mine !== seq) return;
        note.textContent = `Could not compare: ${errorText(e)}`;
      }
    }

    go.onclick = () => void load();
    word.onkeydown = (e) => {
      if (e.key === "Enter" && !e.isComposing) {
        e.preventDefault();
        void load();
      }
    };
    windowPick.onchange = () => void load();
    minPick.onchange = () => void load();
    scopePicker.onchange = () => {
      scope = scopePicker.value;
      void load();
    };
    copy.onclick = () => {
      if (data === null || data.rows.length === 0) return;
      const n = data.rows.length;
      void writeText(similarTsv(data)).then(
        () => showToast({ tone: "info", lines: [`Copied ${n} rows. Paste them into a spreadsheet.`] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };

    const unprovide = provide("similar", () => [
      { id: "similar:find", title: "Find words used like a word", group: "Similar words", words: ["thesaurus", "synonyms", "distributional", "similar", "sketch engine"], run: () => word.focus() },
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
        el.replaceChildren(emptyState("Similar words", "Open a project to see which words are used alike.", "The wug is matching words by the company they keep."));
        return;
      }
      if (!sameProject) {
        scope = "";
        word.value = "";
        data = null;
      }
      drawChips();
      drawScopes();
      el.replaceChildren(h("div", { className: "cc-bar" }, word, go), chips, h("div", { className: "cc-bar" }, windowPick, minPick, scopePicker, copy), note, h("div", { className: "fq-rows" }, table));
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
