// Collexemes (Scrolls): which words fill one slot of a construction more
// often than chance, Stefanowitsch and Gries's collexeme analysis over a
// CQP query. A row opens that word's lines in the Concordance. Rules in
// ../collexemes/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { collexemes, errorText } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { collexemeNote, collexemeTsv, narrowed, SIGNIFICANT, strengthCell, type Collexemes } from "../collexemes/logic";
import { dataGeneration, getState, showToast } from "../store";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

const EXAMPLE = '[lemma="give"] [] [upos="NOUN"]';

export const collexemesPanel: PanelDef = {
  title: "Collexemes",
  create(): PanelView {
    const el = h("div", { className: "panel panel-collexemes" });
    let data: Collexemes | null = null;
    let asked = { query: "", slot: 1, lemma: true, text: "" };
    let seq = 0;
    const query = h("input", { type: "search", className: "cx-query", placeholder: `A construction as a CQP query, e.g. ${EXAMPLE}`, spellcheck: false });
    query.setAttribute("aria-label", "Construction (CQP query)");
    const go = h("button", { type: "button", className: "primary", textContent: "Count" });
    const slot = h("select", { className: "cx-slot", title: "Which position of the construction to count the words in" }, ...Array.from({ length: 8 }, (_, i) => h("option", { value: String(i + 1), textContent: `Position ${i + 1}` })));
    slot.setAttribute("aria-label", "Position");
    const by = h("select", { className: "cx-by", title: "Count lemmas (every form together) or words as written" }, h("option", { value: "lemma", textContent: "Lemmas" }), h("option", { value: "word", textContent: "Words" }));
    by.setAttribute("aria-label", "Count");
    const textPicker = h("select", { title: "Search the whole project or one text (a :: condition in the query picks texts by detail)" });
    textPicker.setAttribute("aria-label", "Where to look");
    const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy the list as a table to paste into a spreadsheet" });
    const note = h("p", { className: "note fq-count" });
    const table = h("table", { className: "fq-table cx-table" });

    const drawTexts = () => {
      const s = getState();
      const keep = textPicker.value;
      textPicker.replaceChildren(h("option", { value: "", textContent: "Whole project" }), ...(s.kind === "open" ? s.texts.map((t) => h("option", { value: t.id, textContent: t.title })) : []));
      textPicker.value = [...textPicker.options].some((o) => o.value === keep) ? keep : "";
    };

    const drawRows = () => {
      if (data === null) {
        table.replaceChildren();
        return;
      }
      note.textContent = collexemeNote(data, asked.query, asked.slot, asked.lemma);
      copy.disabled = data.rows.length === 0;
      // Offer only the positions the matches have.
      for (const o of slot.options) o.disabled = data.positions > 0 && Number(o.value) > data.positions;
      if (data.rows.length === 0) {
        table.replaceChildren();
        return;
      }
      const text = asked.text === "" ? null : { id: asked.text, title: textPicker.selectedOptions[0]?.textContent ?? "" };
      table.replaceChildren(
        h(
          "thead",
          {},
          h(
            "tr",
            {},
            ...[
              [asked.lemma ? "Lemma" : "Word", ""],
              ["In the slot", "Times it fills this position"],
              ["Freq", "Times it occurs in the sentences searched"],
              ["Expected", "Times it would fill the position by chance"],
              ["Strength", "Collostruction strength: −log10 of Fisher's exact p; 1.3 or more is p < .05, below zero the word keeps away"],
            ].map(([t, tip]) => h("th", { scope: "col", textContent: t, title: tip })),
          ),
        ),
        h(
          "tbody",
          {},
          ...data.rows.map((r) => {
            const tr = h(
              "tr",
              { tabIndex: 0, className: r.strength >= SIGNIFICANT ? "cx-drawn" : r.strength < 0 ? "co-neg" : "", title: `Log-likelihood ${r.logLikelihood.toFixed(2)}. Read these lines in the Concordance` },
              h("td", { className: "fq-item", textContent: r.item }),
              h("td", { className: "fq-num", textContent: r.together.toLocaleString("en") }),
              h("td", { className: "fq-num", textContent: r.freq.toLocaleString("en") }),
              h("td", { className: "fq-num", textContent: r.expected.toFixed(2) }),
              h("td", { className: "fq-num", textContent: strengthCell(r.strength) }),
            );
            const open = () => askConcordance(narrowed(asked.query, asked.slot, r.item, asked.lemma) ?? asked.query, "query", text, true, null);
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
      const q = query.value.trim();
      const mine = ++seq;
      data = null;
      table.replaceChildren();
      copy.disabled = true;
      if (q === "") {
        note.textContent = `Type a construction as a CQP query and pick the position to study: which verbs take a ditransitive, which nouns follow an adjective. Try ${EXAMPLE} with position 3. It needs tagged texts (the Sentence panel's tagging, CoNLL-U, TEI or CHAT %mor).`;
        return;
      }
      note.textContent = "Counting…";
      const ask = { query: q, slot: Number(slot.value), lemma: by.value === "lemma", text: textPicker.value };
      try {
        const r = await collexemes({ query: ask.query, slot: ask.slot, lemma: ask.lemma, textId: ask.text === "" ? null : ask.text });
        if (mine !== seq) return;
        data = r;
        asked = ask;
        drawRows();
      } catch (e) {
        if (mine !== seq) return;
        note.textContent = `Could not count: ${errorText(e)}`;
      }
    }

    go.onclick = () => void load();
    query.onkeydown = (e) => {
      if (e.key === "Enter" && !e.isComposing) {
        e.preventDefault();
        void load();
      }
    };
    slot.onchange = by.onchange = textPicker.onchange = () => void load();
    copy.onclick = () => {
      if (data === null || data.rows.length === 0) return;
      const n = data.rows.length;
      void writeText(collexemeTsv(data)).then(
        () => showToast({ tone: "info", lines: [`Copied ${n} rows. Paste them into a spreadsheet.`] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };

    const unprovide = provide("collexemes", () => [
      { id: "collexemes:find", title: "Find the words a construction attracts", group: "Collexemes", words: ["collostructional", "collexeme", "construction", "gries", "slot", "cqp"], run: () => query.focus() },
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
        el.replaceChildren(emptyState("Collexemes", "Open a project to see which words a construction attracts.", "The wug is sorting words into slots."));
        return;
      }
      if (!sameProject) {
        query.value = "";
        data = null;
      }
      drawTexts();
      el.replaceChildren(h("div", { className: "cc-bar" }, query, go), h("div", { className: "cc-bar" }, slot, by, textPicker, copy), note, h("div", { className: "fq-rows" }, table));
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
