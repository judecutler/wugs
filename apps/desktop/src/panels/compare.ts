// Compare word lists: the project's answers to one of its concept lists
// beside other languages' words for the same concepts, read from their own
// files (src-tauri/src/compare.rs), each with how alike it is to the
// project's word, and the whole table copied as an EDICTOR/LingPy file.
// Nothing is written to the project; loaded lists last until the project
// changes. Rules in ../compare/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { compareRead, conceptLists, conceptListView, errorText, projectLanguage, type ListSummary } from "../api";
import { correspondenceLine, correspondences, correspondenceTsv } from "../compare/correspondences";
import { CLOSE, compareLine, compareRows, edictorTsv, languagesOf, type OtherForm, type OurConcept, type Row } from "../compare/logic";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { dataGeneration, getState, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";

export const comparePanel: PanelDef = {
  title: "Compare word lists",
  create(): PanelView {
    const el = h("div", { className: "panel panel-compare" });
    let shownFor = "";
    let lists: ListSummary[] = [];
    let listId = "";
    let concepts: OurConcept[] = [];
    let forms: OtherForm[] = [];
    let rows: Row[] = [];
    let seq = 0;

    const picker = h("select", { className: "cmp-list", title: "Which of the project's concept lists to compare" });
    picker.setAttribute("aria-label", "Concept list");
    const load = h("button", { type: "button", className: "primary", textContent: "Load a word list…", title: "Read another language's words from a CSV or tab-separated table, an EDICTOR file or a CLDF forms.csv" });
    const clear = h("button", { type: "button", textContent: "Clear", title: "Take the loaded lists away" });
    const answeredBox = h("input", { type: "checkbox", checked: true });
    const answered = h("label", { title: "List only the concepts the project has an answer for" }, answeredBox, " Only answered concepts");
    const copy = h("button", { type: "button", textContent: "Copy for EDICTOR", title: "Copy every form, yours and theirs, as an EDICTOR/LingPy wordlist (ID, DOCULECT, CONCEPT, FORM, TOKENS)" });
    const line = h("p", { className: "note cmp-line" });
    const table = h("table", { className: "fq-table cmp-table" });
    // Sound correspondences with one language at a time.
    const corrLang = h("select", { className: "cmp-corr-lang", title: "Which language to align with the project's words" });
    corrLang.setAttribute("aria-label", "Language");
    const corrMin = h("select", { className: "cmp-corr-min", title: "Only pairs of words at least this alike are aligned; lower takes in more cognates and more chance resemblances" }, ...[0.3, 0.4, 0.5, 0.6, 0.7].map((v) => h("option", { value: String(v), textContent: `${Math.round(v * 100)}% alike or more`, selected: v === 0.5 })));
    corrMin.setAttribute("aria-label", "Pairs aligned");
    const differBox = h("input", { type: "checkbox", checked: true });
    const differ = h("label", { title: "Leave out a sound that stays the same" }, differBox, " Only where the sounds differ");
    const corrCopy = h("button", { type: "button", textContent: "Copy table", title: "Copy every correspondence with its count and concepts" });
    const corrLine = h("p", { className: "note cmp-corr-line" });
    const corrTable = h("table", { className: "fq-table cmp-corr" });
    const corrBlock = h(
      "section",
      { className: "cmp-corr-block" },
      h("h3", { textContent: "Sound correspondences" }),
      h("p", { className: "note", textContent: "Each concept's closest pair of words, aligned sound by sound and counted: a correspondence that recurs across many concepts is evidence, one met once may be chance." }),
      h("div", { className: "cc-bar" }, corrLang, corrMin, differ, corrCopy),
      corrLine,
      corrTable,
    );

    const draw = () => {
      rows = compareRows(concepts, forms);
      const langs = languagesOf(forms);
      line.textContent = lists.length === 0 ? "The project has no concept list yet: load one in the Elicit panel's Word lists." : compareLine(rows, langs, forms);
      clear.disabled = forms.length === 0;
      copy.disabled = forms.length === 0;
      const shown = answeredBox.checked ? rows.filter((r) => r.ours.length > 0) : rows;
      const cell = (cells: { form: string; similarity: number | null }[] | undefined) =>
        h(
          "td",
          { className: "ipa" },
          ...(cells ?? []).map((c) =>
            h(
              "span",
              { className: `cmp-form${(c.similarity ?? 0) >= CLOSE ? " cmp-close" : ""}`, title: c.similarity === null ? "" : `${Math.round(c.similarity * 100)}% alike, by sounds` },
              c.form,
              c.similarity === null ? "" : h("small", { textContent: ` ${Math.round(c.similarity * 100)}%` }),
            ),
          ),
        );
      table.replaceChildren(
        h("thead", {}, h("tr", {}, h("th", { scope: "col", textContent: "#" }), h("th", { scope: "col", textContent: "Concept" }), h("th", { scope: "col", textContent: "This project" }), ...langs.map((l) => h("th", { scope: "col", textContent: l })))),
        h(
          "tbody",
          {},
          ...shown.map((r) =>
            h(
              "tr",
              {},
              h("td", { className: "num", textContent: String(r.concept.number) }),
              h("td", { textContent: r.concept.gloss }),
              h("td", { className: "ipa cmp-ours", textContent: r.ours.join(", ") }),
              ...langs.map((l) => cell(r.others.get(l))),
            ),
          ),
        ),
      );
      drawCorrespondences();
    };

    const drawCorrespondences = () => {
      const langs = languagesOf(forms);
      corrBlock.hidden = langs.length === 0;
      if (langs.length === 0) return;
      const pick = langs.includes(corrLang.value) ? corrLang.value : langs[0]!;
      corrLang.replaceChildren(...langs.map((l) => h("option", { value: l, textContent: l, selected: l === pick })));
      const min = Number(corrMin.value);
      const c = correspondences(rows, pick, min);
      corrLine.textContent = correspondenceLine(c, pick, min);
      corrCopy.onclick = () =>
        void writeText(correspondenceTsv(c, pick)).then(
          () => showToast({ tone: "info", lines: [`Copied ${c.list.length} correspondences with ${pick}.`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
      const shown = differBox.checked ? c.list.filter((x) => !x.same) : c.list;
      corrTable.replaceChildren(
        h("thead", {}, h("tr", {}, h("th", { scope: "col", textContent: "This project" }), h("th", { scope: "col", textContent: pick }), h("th", { scope: "col", textContent: "Count" }), h("th", { scope: "col", textContent: "Met in" }))),
        h(
          "tbody",
          {},
          ...shown.map((x) =>
            h(
              "tr",
              { className: !x.same && x.count >= 2 ? "cmp-recurs" : "" },
              h("td", { className: "ipa", textContent: x.ours }),
              h("td", { className: "ipa", textContent: x.theirs }),
              h("td", { className: "num", textContent: String(x.count) }),
              h("td", { textContent: x.concepts.join(", ") }),
            ),
          ),
        ),
      );
    };

    const loadConcepts = async () => {
      const mine = ++seq;
      if (listId === "") {
        concepts = [];
        draw();
        return;
      }
      try {
        const v = await conceptListView({ listId });
        if (mine !== seq) return;
        concepts = v.concepts.map((c) => ({ id: c.id, number: c.number, gloss: c.gloss, concepticonGloss: c.concepticonGloss, answers: c.answers.map((a) => ({ lemma: a.lemma })) }));
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      draw();
    };

    picker.onchange = () => {
      listId = picker.value;
      void loadConcepts();
    };
    answeredBox.onchange = draw;
    corrLang.onchange = drawCorrespondences;
    corrMin.onchange = drawCorrespondences;
    differBox.onchange = drawCorrespondences;
    clear.onclick = () => {
      forms = [];
      draw();
    };
    load.onclick = async () => {
      const picked = await pickFile({ multiple: true, title: "Another language's word list", filters: [{ name: "Word lists", extensions: ["csv", "tsv", "tab", "txt"] }] });
      const paths = picked === null ? [] : Array.isArray(picked) ? picked : [picked];
      const lines: string[] = [];
      for (const path of paths) {
        try {
          const r = await compareRead({ path });
          forms = forms.concat(r.forms);
          lines.push(`${r.name}: ${r.forms.length} forms of ${languagesOf(r.forms).length} ${languagesOf(r.forms).length === 1 ? "language" : "languages"} (columns ${r.columns.join(", ")})${r.skipped > 0 ? `, ${r.skipped} rows without a concept or form skipped` : ""}.`);
        } catch (e) {
          lines.push(`${path}: ${errorText(e)}`);
        }
      }
      if (lines.length > 0) showToast({ tone: "info", lines });
      draw();
    };
    copy.onclick = async () => {
      const lang = await projectLanguage().catch(() => null);
      void writeText(edictorTsv(rows, lang?.name ?? "")).then(
        () => showToast({ tone: "info", lines: ["Copied the word lists as an EDICTOR file. Paste it into a .tsv file and open it at edictor.org, or read it with LingPy."] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };

    const unprovide = provide("compare", () => [
      { id: "compare:load", title: "Load another language's word list to compare", group: "Compare word lists", words: ["cognate", "comparative", "historical", "edictor", "lingpy", "swadesh"], run: () => load.click() },
    ]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}` : "closed";
      if (key === shownFor) return;
      const sameProject = s.kind === "open" && shownFor.split("|")[0] === s.info.projectId;
      shownFor = key;
      if (s.kind !== "open") {
        el.replaceChildren(emptyState("Compare word lists", "Open a project to compare its word lists.", "The wug is learning its cousins' words."));
        return;
      }
      if (!sameProject) {
        forms = [];
        listId = "";
        el.replaceChildren(h("div", { className: "cc-bar" }, picker, load, clear, answered, copy), line, h("div", { className: "cmp-rows" }, table, corrBlock));
      }
      void conceptLists().then(
        (l) => {
          lists = l;
          if (!lists.some((x) => x.id === listId)) listId = lists[0]?.id ?? "";
          picker.replaceChildren(...lists.map((x) => h("option", { value: x.id, textContent: `${x.name} (${x.answered} of ${x.total})`, selected: x.id === listId })));
          void loadConcepts();
        },
        (e) => showToast({ tone: "error", lines: [errorText(e)] }),
      );
    };
    return { element: el, render, dispose: unprovide };
  },
};
