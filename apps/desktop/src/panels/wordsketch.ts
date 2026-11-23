// Word sketch: a lemma's company by grammatical relation, from the parsed
// sentences' dependency trees (src-tauri/src/wordsketch.rs), as Sketch
// Engine shows it. A collocate opens its lines in the Concordance as a CQP
// query. Rules and words in ../wordsketch/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, wordSketch } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { dataGeneration, getState, showToast } from "../store";
import { groupTitle, pairQuery, sketchLine, sketchTsv, type WordSketch } from "../wordsketch/logic";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

export const wordSketchPanel: PanelDef = {
  title: "Word sketch",
  create(): PanelView {
    const el = h("div", { className: "panel panel-wordsketch" });
    let data: WordSketch | null = null;
    let asked = "";
    let seq = 0;
    let shownFor = "";
    const field = h("input", { type: "search", className: "ws-lemma", placeholder: "A lemma, such as eat", spellcheck: false });
    field.setAttribute("aria-label", "Lemma to sketch");
    const go = h("button", { type: "button", className: "primary", textContent: "Sketch" });
    const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy every relation and collocate with its count and logDice" });
    const line = h("p", { className: "note ws-line" });
    const grid = h("div", { className: "ws-grid" });

    const draw = () => {
      copy.disabled = data === null || data.groups.length === 0;
      if (data === null) {
        line.textContent = "Type a lemma to see its subjects, objects, modifiers and the words it attaches to, read from the parsed sentences' dependency trees.";
        grid.replaceChildren();
        return;
      }
      const w = data;
      line.textContent = sketchLine(w, asked);
      grid.replaceChildren(
        ...w.groups.map((g) =>
          h(
            "section",
            { className: "ws-group" },
            h("h3", { className: "ws-head", textContent: groupTitle(g, w.lemma) }, h("span", { className: "ws-total", textContent: ` ${g.total}` })),
            h(
              "table",
              { className: "ws-items" },
              h("thead", {}, h("tr", {}, h("th", { scope: "col", textContent: "" }), h("th", { scope: "col", className: "num", textContent: "n", title: "Times together in this relation" }), h("th", { scope: "col", className: "num", textContent: "logDice", title: "Sketch Engine's association score: 14 when the two are always together" }))),
              h(
                "tbody",
                {},
                ...g.items.map((i) => {
                  const b = h("button", { type: "button", className: "link-button", textContent: i.lemma, title: `Open the lines with «${w.lemma}» and «${i.lemma}» in the Concordance` });
                  b.onclick = () => askConcordance(pairQuery(w.lemma, i), "query");
                  return h("tr", {}, h("td", {}, b), h("td", { className: "num", textContent: String(i.count) }), h("td", { className: "num", textContent: i.logDice.toFixed(1) }));
                }),
              ),
            ),
          ),
        ),
      );
    };

    const run = async (lemma = field.value.trim()) => {
      if (lemma === "") return;
      const mine = ++seq;
      asked = lemma;
      go.disabled = true;
      try {
        const r = await wordSketch({ lemma });
        if (mine !== seq) return;
        data = r;
      } catch (e) {
        if (mine !== seq) return;
        data = null;
        showToast({ tone: "error", lines: [errorText(e)] });
      } finally {
        if (mine === seq) go.disabled = false;
      }
      draw();
    };

    go.onclick = () => void run();
    field.onkeydown = (e) => {
      if (e.key === "Enter" && !e.isComposing) void run();
    };
    copy.onclick = () => {
      if (data === null) return;
      void writeText(sketchTsv(data)).then(
        () => showToast({ tone: "info", lines: ["Copied the word sketch."] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };

    const unprovide = provide("wordsketch", () => [
      { id: "wordsketch:focus", title: "Sketch a word", group: "Word sketch", words: ["collocation", "grammatical relations", "dependency", "sketch engine", "subject", "object"], run: () => field.focus() },
    ]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}` : "closed";
      if (key === shownFor) return;
      const sameProject = s.kind === "open" && shownFor.split("|")[0] === s.info.projectId;
      shownFor = key;
      if (s.kind !== "open") {
        seq += 1;
        data = null;
        el.replaceChildren(emptyState("Word sketch", "Open a project to sketch its words.", "The wug is drawing."));
        return;
      }
      if (!sameProject) {
        data = null;
        field.value = asked = "";
        el.replaceChildren(h("div", { className: "cc-bar" }, field, go, copy), line, grid);
        draw();
      } else if (asked !== "") {
        void run(asked);
      }
    };
    return { element: el, render, dispose: unprovide };
  },
};
