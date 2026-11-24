// Word order: the orders WALS describes (subject, object and verb;
// adpositions; adjective, genitive, demonstrative, numeral and relative
// clause with the noun; negation), counted in the parsed sentences, each
// with an example sentence, the order the counts suggest, and the
// project's WALS answer beside it with a click to take the suggestion.
// Counted by src-tauri/src/word_order.rs; rules in ../wordorder/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, setTypologyAnswer, typologyAnswers, wordOrder } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import type { Ours } from "../typology/logic";
import { answerNote, orderLine, orderTsv, total, verdict, type Example, type WordOrder } from "../wordorder/logic";
import { dataGeneration, getState, selectSegment, selectText, showToast } from "../store";
import { SHOW_PANEL, type PanelDef, type PanelView } from "./panel";

export const wordOrderPanel: PanelDef = {
  title: "Word order",
  create(): PanelView {
    const el = h("div", { className: "panel panel-wordorder" });
    let key = "";
    let seq = 0;
    let data: WordOrder | null = null;
    let ours = new Map<string, Ours>();

    const openExample = async (e: Example) => {
      await selectText(e.textId);
      selectSegment({ textId: e.textId, tierId: e.tierId, segmentId: e.segmentId });
      window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "sentence" }));
    };

    const load = async () => {
      const mine = ++seq;
      try {
        const [w, a] = await Promise.all([wordOrder(), typologyAnswers({ source: "wals" }).catch(() => [] as Ours[])]);
        if (mine !== seq) return;
        data = w;
        ours = new Map(a.map((x) => [x.id, x]));
        paint();
      } catch (e) {
        if (mine === seq) el.replaceChildren(emptyState("Word order", errorText(e)));
      }
    };

    const take = async (id: string, code: string, note: string) => {
      try {
        const r = await setTypologyAnswer({ source: "wals", id, value: code, note });
        ours.set(id, r);
        showToast({ tone: "info", lines: [`Kept as this project's answer to WALS ${id}. The Typology panel shows it under WALS.`] });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      paint();
    };

    const paint = () => {
      if (data === null) {
        el.replaceChildren(emptyState("Word order", "Counting the orders in the parsed sentences…"));
        return;
      }
      const w = data;
      const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy every feature with its counts and the suggested order" });
      copy.onclick = () =>
        void writeText(orderTsv(w)).then(
          () => showToast({ tone: "info", lines: ["Copied the word orders as a table."] }),
          (e) => showToast({ tone: "error", lines: [errorText(e)] }),
        );
      const again = h("button", { type: "button", textContent: "Count again" });
      again.onclick = () => void load();
      const cards = w.parsed === 0
        ? []
        : w.patterns.map((p) => {
            const v = verdict(p);
            const n = total(p);
            const bars = p.orders
              .filter((o) => o.count > 0 || p.orders.length <= 2)
              .map((o) => {
                const ex = o.example;
                const label = ex === null ? h("span", { textContent: o.label }) : h("button", { type: "button", className: "link-button", textContent: o.label, title: `e.g. «${ex.sentence}». Open it in the Sentence panel.` });
                if (ex !== null) label.onclick = () => void openExample(ex);
                return h(
                  "div",
                  { className: "wo-order" },
                  h("span", { className: "wo-label" }, label),
                  (() => {
                    const fill = h("span", { className: "wo-fill" });
                    fill.style.width = `${n === 0 ? 0 : Math.round((100 * o.count) / n)}%`;
                    return h("span", { className: "wo-bar" }, fill);
                  })(),
                  h("span", { className: "num", textContent: o.count.toLocaleString() }),
                );
              });
            const mine = ours.get(p.id);
            const answered = mine !== undefined && mine.value !== "";
            const same = answered && mine.value === v.code;
            const use =
              v.code === null || same
                ? null
                : h("button", { type: "button", textContent: answered ? `Change the answer to ${v.label}` : `Use ${v.label} as the answer`, title: `Write ${v.label} as this project's answer to WALS ${p.id}, with the counts as its note` });
            if (use !== null) use.onclick = () => void take(p.id, v.code!, answerNote(p, w.parsed));
            const answer = !answered ? "No WALS answer yet." : same ? "This project's WALS answer agrees." : `This project's WALS answer is code ${mine.value}.`;
            return h(
              "li",
              { className: "wo-card" },
              h("div", { className: "wo-head" }, h("strong", { textContent: p.title }), h("span", { className: "note", textContent: ` WALS ${p.id}` })),
              ...bars,
              h("p", { className: "note", title: v.why }, h("strong", { textContent: v.label }), ` · ${answer}`),
              use,
            );
          });
      el.replaceChildren(h("div", { className: "cc-bar" }, again, copy), h("p", { className: "note wo-line", textContent: orderLine(w) }), h("ul", { className: "wo-list" }, ...cards));
    };

    const unprovide = provide("wordorder", () => [{ id: "wordorder:again", title: "Count word orders again", group: "Word order", words: ["wals", "typology", "svo", "sov", "basic word order", "adposition", "dependency"], run: () => void load() }]);

    const render = () => {
      const s = getState();
      if (s.kind !== "open") {
        key = "";
        data = null;
        el.replaceChildren(emptyState("Word order", "Open a project to count its word orders."));
        return;
      }
      const wanted = `${s.info.projectId}|${dataGeneration()}`;
      if (wanted === key) return;
      key = wanted;
      if (data === null) paint();
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
