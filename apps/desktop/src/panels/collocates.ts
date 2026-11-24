// Collocates (Scrolls, SCROLLS.md §5 step 6): which words keep company with a
// word, with the scores AntConc and Sketch Engine offer (log-likelihood,
// mutual information, logDice, t-score). A row opens the Concordance on the node, where the pairs
// can be read in context.
import { fillScopes, scopeOf } from "../metadata/scope";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { collocates, errorText } from "../api";
import { asTsv, COLLOCATE_SORTS, compareCollocates, compareLine, compareTsv, leansTo, score, sortCollocates, SPANS, summary, type CollocateSort, type Collocates, type CompareRow } from "../collocates/logic";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { dataGeneration, getState, showToast } from "../store";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

type Kind = "word" | "translation";
const KIND_CHIPS: { id: Kind; label: string }[] = [
  { id: "word", label: "Words" },
  { id: "translation", label: "Translations" },
];
const MINIMUMS = [1, 2, 3, 5, 10];
/** Rows drawn at once; Copy table has them all. */
const DRAWN = 500;

const picker = (label: string, title: string, values: readonly number[], value: number, word: (n: number) => string) => {
  const s = h("select", { title }, ...values.map((n) => h("option", { value: String(n), textContent: word(n) })));
  s.setAttribute("aria-label", label);
  s.value = String(value);
  return s;
};

export const collocatesPanel: PanelDef = {
  title: "Collocates",
  create(): PanelView {
    const el = h("div", { className: "panel panel-collocates" });
    let kind: Kind = "word";
    let sortBy: CollocateSort = "ll";
    let scope = "";
    let data: Collocates | null = null;
    let asked = "";
    /** The second word and its collocates, while comparing two words. */
    let other: { word: string; data: Collocates; rows: CompareRow[] } | null = null;
    let seq = 0;
    const node = h("input", { type: "search", className: "co-node", placeholder: "A word, or a pattern like walk*", spellcheck: false });
    node.setAttribute("aria-label", "Word to find collocates of");
    const go = h("button", { type: "button", className: "primary", textContent: "Find collocates" });
    const versus = h("input", { type: "search", className: "co-versus", placeholder: "Compare with another word", spellcheck: false, title: "Set two words' collocates side by side, as Sketch Engine's word sketch difference does: strong against powerful, or two spellings of one word" });
    versus.setAttribute("aria-label", "Compare with");
    const left = picker("Words before", "How many words before it to look at", SPANS, 5, (n) => `${n} before`);
    const right = picker("Words after", "How many words after it to look at", SPANS, 5, (n) => `${n} after`);
    const min = picker("Minimum together", "Leave out words met together fewer times than this", MINIMUMS, 2, (n) => (n === 1 ? "Every pair" : `Together ${n}+ times`));
    const chips = h("div", { className: "scopes", role: "radiogroup", ariaLabel: "Look among" });
    const scopePicker = h("select", { title: "Look in the whole project, one text, or the texts with one detail (add details in the Texts panel)" });
    scopePicker.setAttribute("aria-label", "Where to look");
    const sorter = h("select", { className: "co-sort", title: "Order of the rows. Windows stop at a segment's edge, so numbers are close to AntConc's but not equal on the same file." }, ...COLLOCATE_SORTS.map((x) => h("option", { value: x.id, textContent: x.label })));
    sorter.setAttribute("aria-label", "Sort");
    const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy the list as a table to paste into a spreadsheet" });
    const note = h("p", { className: "note fq-count" });
    const table = h("table", { className: "fq-table co-table" });

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

    const drawCompare = (a: Collocates, b: { word: string; data: Collocates; rows: CompareRow[] }) => {
      const rows = b.rows;
      note.textContent = compareLine(asked, b.word, a, b.data, rows) + (rows.length > DRAWN ? ` The first ${DRAWN} are drawn (Copy table has them all).` : "");
      table.className = "fq-table co-table ws-diff";
      sorter.hidden = true;
      const head = h(
        "tr",
        {},
        ...[
          ["Collocate", ""],
          [`«${asked}»`, `logDice with «${asked}» (– when it is no collocate of it)`],
          [`«${b.word}»`, `logDice with «${b.word}»`],
          ["Difference", "The first logDice less the second: high for collocates of the first word, low for the second's"],
        ].map(([t, tip]) => h("th", { textContent: t, scope: "col", title: tip })),
      );
      const sc = scopeOf(scope);
      const text = sc.textId === null ? null : { id: sc.textId, title: scopePicker.selectedOptions[0]?.textContent ?? "" };
      const cell = (r: CompareRow["a"]) => (r === null ? "–" : `${score(r.logDice)} (${r.together})`);
      table.replaceChildren(
        h("thead", {}, head),
        h(
          "tbody",
          {},
          ...rows.slice(0, DRAWN).map((r) => {
            const lean = leansTo(r);
            const word = lean === "b" ? b.word : asked;
            const tr = h(
              "tr",
              { tabIndex: 0, className: lean === "both" ? "" : `ws-lean-${lean}`, title: `logDice (times together). Read «${word}» in context in the Concordance` },
              h("td", { className: "fq-item", textContent: r.item }),
              h("td", { className: "fq-num", textContent: cell(r.a) }),
              h("td", { className: "fq-num", textContent: cell(r.b) }),
              h("td", { className: "fq-num", textContent: score(r.diff) }),
            );
            tr.onclick = () => askConcordance(word, kind, text, false, sc.within);
            tr.onkeydown = (e) => {
              if (e.key === "Enter") askConcordance(word, kind, text, false, sc.within);
            };
            return tr;
          }),
        ),
      );
    };

    const drawRows = () => {
      if (data === null) {
        table.replaceChildren();
        return;
      }
      if (other !== null) {
        drawCompare(data, other);
        return;
      }
      table.className = "fq-table co-table";
      sorter.hidden = false;
      const rows = sortCollocates(data.rows, sortBy);
      const cut = rows.length > DRAWN ? ` The first ${DRAWN} are drawn (Copy table has them all).` : "";
      note.textContent = summary(asked, data, Number(left.value), Number(right.value)) + cut;
      const head = h(
        "tr",
        {},
        ...[
          ["Collocate", ""],
          ["Near", "Times it occurs within the window (each word counted once even where windows overlap); hover a row for before / after"],
          ["Freq", "Times it occurs anywhere here"],
          ["LL", "Log-likelihood: how surprising the pairing is; below zero means rarer than chance"],
          ["MI", "Mutual information: high for pairs that rarely occur apart"],
          ["Dice", "logDice, Sketch Engine's default: up to 14, comparable between corpora of different sizes"],
        ].map(([t, tip]) => h("th", { textContent: t, scope: "col", title: tip })),
      );
      const sc = scopeOf(scope);
      const text = sc.textId === null ? null : { id: sc.textId, title: scopePicker.selectedOptions[0]?.textContent ?? "" };
      table.replaceChildren(
        h("thead", {}, head),
        h(
          "tbody",
          {},
          ...rows.slice(0, DRAWN).map((r) => {
            const tr = h(
              "tr",
              { tabIndex: 0, title: `${r.left} before, ${r.right} after; t-score ${score(r.tScore)}. Read «${asked}» in context in the Concordance (sort by the word before or after to bring «${r.item}» together)` },
              h("td", { className: "fq-item", textContent: r.item }),
              h("td", { className: "fq-num", textContent: r.together.toLocaleString("en") }),
              h("td", { className: "fq-num", textContent: r.freq.toLocaleString("en") }),
              h("td", { className: r.logLikelihood < 0 ? "fq-num co-neg" : "fq-num", textContent: score(r.logLikelihood) }),
              h("td", { className: "fq-num", textContent: score(r.mutualInformation) }),
              h("td", { className: "fq-num", textContent: score(r.logDice) }),
            );
            const open = () => askConcordance(asked, kind, text, false, sc.within);
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
      const query = node.value.trim();
      const mine = ++seq;
      data = null;
      other = null;
      table.replaceChildren();
      if (query === "") {
        note.textContent = "Type a word to see which words keep it company, scored the way AntConc and Sketch Engine score collocations.";
        return;
      }
      note.textContent = "Counting…";
      try {
        const ask = (word: string) => collocates({ node: word, kind, left: Number(left.value), right: Number(right.value), minTogether: Number(min.value), textId: scopeOf(scope).textId, within: scopeOf(scope).within });
        const second = versus.value.trim();
        const [r, r2] = await Promise.all([ask(query), second === "" ? Promise.resolve(null) : ask(second)]);
        if (mine !== seq) return;
        data = r;
        asked = query;
        other = r2 === null ? null : { word: second, data: r2, rows: compareCollocates(r.rows, r2.rows) };
        drawRows();
      } catch (e) {
        if (mine !== seq) return;
        note.textContent = `Could not count: ${errorText(e)}`;
      }
    }

    go.onclick = () => void load();
    node.onkeydown = versus.onkeydown = (e) => {
      if (e.key === "Enter" && !e.isComposing) {
        e.preventDefault();
        void load();
      }
    };
    // Clearing the second word (its × or emptied by hand) goes back to one word's list.
    versus.addEventListener("search", () => {
      if (versus.value === "") void load();
    });
    for (const s of [left, right, min]) s.onchange = () => void load();
    scopePicker.onchange = () => {
      scope = scopePicker.value;
      void load();
    };
    sorter.onchange = () => {
      sortBy = sorter.value as CollocateSort;
      drawRows();
    };
    copy.onclick = () => {
      if (data !== null && other !== null) {
        const rows = other.rows;
        void writeText(compareTsv(asked, other.word, rows)).then(
          () => showToast({ tone: "info", lines: [`Copied ${rows.length} rows. Paste them into a spreadsheet.`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
        return;
      }
      if (data === null || data.rows.length === 0) return;
      const rows = sortCollocates(data.rows, sortBy);
      void writeText(asTsv(rows)).then(
        () => showToast({ tone: "info", lines: [`Copied ${rows.length} rows. Paste them into a spreadsheet.`] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };

    const unprovide = provide("collocates", () => [
      { id: "collocates:find", title: "Find collocates of a word", group: "Collocates", words: ["collocation", "co-occurrence", "log-likelihood", "mutual information", "antconc"], run: () => node.focus() },
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
        el.replaceChildren(emptyState("Collocates", "Open a project to see which words keep company with a word.", "The wug is listening for who sits with whom."));
        return;
      }
      if (!sameProject) {
        scope = "";
        node.value = "";
        versus.value = "";
        data = null;
        other = null;
      }
      drawChips();
      drawScopes();
      el.replaceChildren(
        h("div", { className: "cc-bar" }, node, go, versus),
        chips,
        h("div", { className: "cc-bar" }, left, right, min, scopePicker),
        h("div", { className: "cc-bar" }, sorter, copy),
        note,
        h("div", { className: "fq-rows" }, table),
      );
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
