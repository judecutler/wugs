// Affixes: the project's affixes and clitics read from its glossed words
// (src-tauri/src/affixes.rs), as the position-class chart a grammar draws
// and a table behind it. Words and tables in ../affixes/logic.ts. Read-only:
// an affix opens the Concordance on it, an example opens its sentence.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, projectAffixes, soundWords } from "../api";
import { affixLabel, guessedLine, signatures, signaturesTsv, type Side, type WordCount } from "../affixes/signatures";
import { allomorphSets, allomorphsTsv, setNote, soundsLine } from "../affixes/allomorphs";
import { affixTsv, chartTsv, elsewhere, marked, productivityCell, productivityLine, pairLine, pairNote, pairs, pairsSummary, pairsTsv, slotChart, slotLabel, summaryLine, type Affix, type Affixes } from "../affixes/logic";
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
    const formsNote = h("p", { className: "note" });
    const forms = h("div", { className: "af-forms" });
    const copyForms = h("button", { type: "button", textContent: "Copy forms", title: "Every gloss written more than one way, each form with the sounds next to it and the stem's nearest vowel" });

    let words: WordCount[] | null = null;
    let side: Side = "suffix";
    const guessNote = h("p", { className: "note" });
    const guessTable = h("table", { className: "pg-table af-guess" });
    const copyGuess = h("button", { type: "button", textContent: "Copy table", title: "Every set of endings with its stems" });
    const sidePick = h(
      "select",
      { ariaLabel: "Endings or beginnings", onchange: () => ((side = sidePick.value === "prefix" ? "prefix" : "suffix"), drawGuess()) },
      h("option", { value: "suffix", textContent: "Endings" }),
      h("option", { value: "prefix", textContent: "Beginnings" }),
    );
    const STEMS_SHOWN = 12;

    function drawGuess() {
      if (words === null) {
        guessNote.textContent = "";
        guessTable.replaceChildren();
        copyGuess.disabled = true;
        return;
      }
      const g = signatures(words, side);
      guessNote.textContent = guessedLine(g, side);
      copyGuess.disabled = g.signatures.length === 0;
      guessTable.replaceChildren(
        ...(g.signatures.length === 0
          ? []
          : [
              h("thead", {}, h("tr", {}, ...[side === "suffix" ? "Endings" : "Beginnings", "Stems", "Words", "Stems, most used first"].map((t) => h("th", { scope: "col", textContent: t })))),
              h(
                "tbody",
                {},
                ...g.signatures.map((sig) =>
                  h(
                    "tr",
                    {},
                    h("th", { scope: "row", textContent: sig.affixes.map((a) => affixLabel(a, side)).join(" ") }),
                    h("td", { textContent: String(sig.stems.length) }),
                    h("td", { textContent: String(sig.tokens) }),
                    h(
                      "td",
                      { className: "af-stems" },
                      ...sig.stems.slice(0, STEMS_SHOWN).flatMap((x, i) => {
                        const b = h("button", { type: "button", className: "linkish", textContent: x.stem, title: `Every word ${side === "suffix" ? "starting" : "ending"} with «${x.stem}» in the Concordance` });
                        b.onclick = () => askConcordance(side === "suffix" ? `${x.stem}*` : `*${x.stem}`, "word", null, false);
                        return i === 0 ? [b] : [", ", b];
                      }),
                      ...(sig.stems.length > STEMS_SHOWN ? [h("span", { className: "note", textContent: ` and ${sig.stems.length - STEMS_SHOWN} more` })] : []),
                    ),
                  ),
                ),
              ),
            ]),
      );
    }

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
        forms.replaceChildren();
        formsNote.textContent = "";
        return;
      }
      const sets = allomorphSets(data.affixes);
      copyForms.disabled = sets.length === 0;
      formsNote.textContent =
        sets.length === 0
          ? "No gloss is written more than one way on the same side of the root."
          : `${sets.length} ${sets.length === 1 ? "gloss is" : "glosses are"} written more than one way on the same side of the root. For each form: the sounds right next to it in the words it is in, and the stem's nearest vowel.`;
      forms.replaceChildren(
        ...sets.map((s) =>
          h(
            "div",
            { className: s.tells.length > 0 ? "af-set af-set-told" : "af-set" },
            h("div", { className: "af-set-head" }, gloss(s.gloss), h("span", { className: "note", textContent: ` ${s.kind}, ${s.uses} ${s.uses === 1 ? "use" : "uses"}` })),
            h(
              "div",
              { className: "ag-scroll" },
              h(
              "table",
              { className: "pg-table" },
              h("thead", {}, h("tr", {}, ...["Form", "Uses", s.side === "after" ? "After" : "Before", "Nearest vowel"].map((t) => h("th", { scope: "col", textContent: t })))),
              h(
                "tbody",
                {},
                ...s.members.map((m) => {
                  const b = h("button", { type: "button", className: "linkish", textContent: marked(m.affix), title: "Every word with it in the Concordance" });
                  b.onclick = () => find(m.affix);
                  return h(
                    "tr",
                    {},
                    h("th", { scope: "row" }, b),
                    h("td", { textContent: String(m.affix.uses) }),
                    h("td", { className: s.tells.includes("next") ? "af-tells" : "", textContent: soundsLine(m.next) }),
                    h("td", { className: s.tells.includes("vowel") ? "af-tells" : "", textContent: soundsLine(m.vowel) }),
                  );
                }),
              ),
              ),
            ),
            h("p", { className: "af-note", textContent: setNote(s) }),
          ),
        ),
      );
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
      summary.textContent = [summaryLine(data), productivityLine(data.affixes)].filter((x) => x !== "").join(" ");
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
      const head = ["Affix", "Gloss", "Slot", "Uses", "Roots", "P", "Root categories", "Example"];
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
              h("td", { textContent: productivityCell(a), title: a.hapaxes === undefined ? "" : `In ${a.types} word forms, ${a.hapaxes} of them met only once: Baayen's productivity, the share of its uses that are new words` }),
              h("td", { textContent: a.categories.map(([c, n]) => `${c} ${n}`).join(", ") }),
              h("td", {}, example),
            );
          }),
        ),
      );
    };

    async function load() {
      const mine = ++seq;
      // The guesses need only the words, so they show even before anything is glossed.
      void soundWords({ tierId: null, textId: null }).then(
        (r) => {
          if (mine !== seq) return;
          words = r.words;
          drawGuess();
        },
        (e) => {
          if (mine !== seq) return;
          words = null;
          drawGuess();
          guessNote.textContent = `Could not read the words: ${errorText(e)}`;
        },
      );
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
    copyForms.onclick = () => copy("forms of each gloss", () => (data === null ? null : allomorphsTsv(allomorphSets(data.affixes), marked)));
    copyGuess.onclick = () => copy("sets of endings", () => (words === null ? null : signaturesTsv(signatures(words, side), side)));
    recount.onclick = () => void load();

    const unprovide = provide("affixes", () => [
      { id: "affixes:chart", title: "Copy the position-class chart", group: "Affixes", words: ["slot", "template", "morphology", "affix order"], run: () => copyChart.click() },
      { id: "affixes:forms", title: "Copy the forms of each gloss (allomorphs)", group: "Affixes", words: ["allomorph", "vowel harmony", "assimilation", "conditioning"], run: () => copyForms.click() },
    ]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}` : "closed";
      if (key === shown) return;
      shown = key;
      if (s.kind !== "open") {
        seq += 1;
        data = null;
        words = null;
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
        h("h3", { textContent: "Forms of one gloss" }),
        h("div", { className: "cc-bar" }, copyForms),
        formsNote,
        forms,
        h("h3", { textContent: "Guessed from the words" }),
        h("div", { className: "cc-bar" }, sidePick, copyGuess),
        guessNote,
        h("div", { className: "pg-rows" }, guessTable),
        h("p", { className: "note", textContent: "Read from the transcriptions alone, nothing glossed needed: stems that take the same set of endings (∅ for none), as Goldsmith's Linguistica finds them. A guide to where to look; some sets are chance, and nothing is written." }),
      );
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
