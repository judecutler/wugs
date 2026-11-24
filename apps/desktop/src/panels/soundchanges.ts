// Sound changes: ordered rewrite rules run over the lexicon's headwords or a
// list of words, each word shown with its output and the steps that changed
// it, for historical reconstruction and for testing phonological rules
// against the data. Nothing in the project is changed; the rules are kept
// as a project setting (src-tauri/src/soundchanges.rs). Rules and words in
// ../soundchange/sca.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, lexiconList, setSoundChanges, soundChanges } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { derivationsTsv, derive, EXAMPLE_RULES, expectedLine, pairsOf, parseRules, resultLine, type Derivation } from "../soundchange/sca";
import { dataGeneration, getState, lexiconRevision, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";

/** At most this many rows are drawn (all are copied). */
const SHOWN = 500;

export const soundChangesPanel: PanelDef = {
  title: "Sound changes",
  create(): PanelView {
    const el = h("div", { className: "panel panel-soundchanges" });
    let shownFor = "";
    let headwords: string[] = [];
    let kept = "";
    let derivations: Derivation[] = [];
    let expected: (string | null)[] = [];
    let timer: number | undefined;

    const rulesField = h("textarea", { className: "sca-rules", spellcheck: false, rows: 10, placeholder: "V = a e i o u\np t k > b d g / V_V" });
    rulesField.setAttribute("aria-label", "Rules");
    const problems = h("ul", { className: "sca-problems" });
    const source = h("select", { className: "sca-source", title: "Which words the rules run over" }, h("option", { value: "lexicon", textContent: "The lexicon's headwords" }), h("option", { value: "typed", textContent: "Words typed here" }));
    source.setAttribute("aria-label", "Words");
    const wordsField = h("textarea", { className: "sca-words", spellcheck: false, rows: 4, placeholder: "One word a line, or several on a line; kapa > kaba to check a word against its attested form" });
    wordsField.setAttribute("aria-label", "Words to run");
    wordsField.hidden = true;
    const changedBox = h("input", { type: "checkbox", checked: true });
    const changedOnly = h("label", { title: "List only the words the rules change" }, changedBox, " Only words that change");
    const keep = h("button", { type: "button", textContent: "Keep with the project", title: "Save these rules in the project, so they sync and open here next time" });
    const example = h("button", { type: "button", textContent: "Example rules", title: "Fill in a few rules that show the notation" });
    const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy every word with its output and steps" });
    const line = h("p", { className: "note sca-line" });
    const table = h("table", { className: "fq-table sca-table" });
    const help = h("p", {
      className: "note",
      textContent:
        "A category is a capital and its sounds (V = a e i o u). A rule is sounds > what they become / where: p t k > b d g / V_V. _ is the sound's place, # a word's edge, ( ) optional, ∅ nothing (deletion or, on the left, insertion), // after the place where the rule does not apply. Rules run in order; ; starts a comment.",
    });

    const words = (): { input: string; expected: string | null }[] => (source.value === "typed" ? pairsOf(wordsField.value) : headwords.map((input) => ({ input, expected: null })));

    const run = () => {
      const r = parseRules(rulesField.value);
      problems.replaceChildren(...r.problems.map((p) => h("li", { textContent: `Line ${p.line} ${p.message}.` })));
      const ws = words();
      derivations = ws.map((w) => derive(w.input, r.rules));
      expected = ws.map((w) => w.expected);
      const checking = expected.some((e) => e !== null);
      line.textContent = r.rules.length === 0 && r.problems.length === 0 ? "Write rules above, or start from the example rules." : resultLine(derivations, r) + expectedLine(derivations, expected);
      const rows = derivations.map((d, i) => ({ d, e: expected[i] ?? null }));
      // A word the rules leave alone still shows when it should have changed.
      const listed = changedBox.checked ? rows.filter(({ d, e }) => d.output !== d.input || (e !== null && e !== d.output)) : rows;
      table.replaceChildren(
        h("thead", {}, h("tr", {}, ...["Word", "Becomes", ...(checking ? ["Expected"] : []), "Steps"].map((c) => h("th", { scope: "col", textContent: c })))),
        h(
          "tbody",
          {},
          ...listed.slice(0, SHOWN).map(({ d, e }) =>
            h(
              "tr",
              { className: e !== null && e !== d.output ? "sca-miss" : d.output === d.input ? "sca-same" : "" },
              h("td", { className: "ipa", textContent: d.input }),
              h("td", { className: "ipa sca-out", textContent: d.output === "" ? "∅" : d.output }),
              ...(checking ? [h("td", { className: "ipa", textContent: e === null ? "" : `${e === "" ? "∅" : e} ${e === d.output ? "✓" : "✗"}` })] : []),
              h("td", { className: "ipa sca-steps", textContent: d.steps.map((s) => `${s.line}: ${s.form || "∅"}`).join(" → ") }),
            ),
          ),
        ),
      );
      if (listed.length > SHOWN) line.textContent += ` The first ${SHOWN} are listed; Copy table copies all.`;
      keep.disabled = rulesField.value.trim() === kept.trim();
    };
    const soon = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(run, 250);
    };

    rulesField.oninput = soon;
    wordsField.oninput = soon;
    changedBox.onchange = run;
    source.onchange = () => {
      wordsField.hidden = source.value !== "typed";
      run();
    };
    example.onclick = () => {
      rulesField.value = rulesField.value.trim() === "" ? EXAMPLE_RULES : `${rulesField.value.trimEnd()}\n\n${EXAMPLE_RULES}`;
      run();
    };
    keep.onclick = async () => {
      try {
        kept = await setSoundChanges({ rules: rulesField.value });
        showToast({ tone: "info", lines: ["Kept the rules with the project."] });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      run();
    };
    copy.onclick = () =>
      void writeText(derivationsTsv(derivations, expected)).then(
        () => showToast({ tone: "info", lines: [`Copied ${derivations.length} words with their outputs.`] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );

    const unprovide = provide("soundchanges", () => [
      { id: "sca:keep", title: "Keep the sound changes with the project", group: "Sound changes", words: ["historical", "reconstruction", "rules", "phonology", "sca"], run: () => keep.click() },
    ]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}|${lexiconRevision()}` : "closed";
      if (key === shownFor) return;
      const sameProject = s.kind === "open" && shownFor.split("|")[0] === s.info.projectId;
      shownFor = key;
      if (s.kind !== "open") {
        el.replaceChildren(emptyState("Sound changes", "Open a project to run sound changes over its words.", "The wug is practising its vowels."));
        return;
      }
      if (!sameProject) {
        rulesField.value = "";
        wordsField.value = "";
        kept = "";
        el.replaceChildren(
          h("div", { className: "sca-top" }, h("div", { className: "sca-col" }, rulesField, problems, help), h("div", { className: "sca-col" }, h("div", { className: "cc-bar" }, source, changedOnly), wordsField, h("div", { className: "cc-bar" }, example, keep, copy))),
          line,
          h("div", { className: "sca-rows" }, table),
        );
        void soundChanges().then(
          (r) => {
            kept = r;
            if (rulesField.value === "") rulesField.value = r;
            run();
          },
          () => run(),
        );
      }
      void lexiconList({ query: "" }).then(
        (rows) => {
          headwords = [...new Set(rows.map((r) => r.lemma.trim()).filter((l) => l !== ""))];
          run();
        },
        () => run(),
      );
    };
    return { element: el, render, dispose: unprovide };
  },
};
