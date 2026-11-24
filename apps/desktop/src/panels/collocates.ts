// Collocates (Scrolls, SCROLLS.md §5 step 6): which words keep company with a
// word, with the scores AntConc and Sketch Engine offer (log-likelihood,
// mutual information, logDice, t-score). A row opens the Concordance on the node, where the pairs
// can be read in context.
import type { Where } from "../metadata/logic";
import { fillScopes, scopeOf } from "../metadata/scope";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { collocates, errorText, saveDocument } from "../api";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { standalone } from "./vowels";
import { network } from "../collocates/network";
import { lemmaQuery, asTsv, COLLOCATE_SORTS, compareCollocates, compareLine, compareTsv, leansTo, score, sortCollocates, SPANS, summary, type CollocateSort, type Collocates, type CompareRow } from "../collocates/logic";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { dataGeneration, getState, showToast } from "../store";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

type Kind = "word" | "translation" | "lemma";
const KIND_CHIPS: { id: Kind; label: string }[] = [
  { id: "word", label: "Words" },
  { id: "translation", label: "Translations" },
  { id: "lemma", label: "Lemmas" },
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
    // A row opens the Concordance on the node: a lemma as the CQP query that finds it.
    const ask = (node: string, text: { id: string; title: string } | null, within: Where | null) =>
      kind === "lemma" ? askConcordance(lemmaQuery(node, within), "query", text, true, null) : askConcordance(node, kind, text, false, within);
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
    // The network view (LancsBox's GraphColl): the word, its collocates around it, a second word's beside.
    let netOn = localStorage.getItem("wugs.collocates.network") === "on";
    const netToggle = h("button", { type: "button", textContent: "Network", title: "Draw the strongest collocates around the word, stronger nearer; click one to set it beside the word and see what they share" });
    const netBox = h("div", { className: "co-net" });
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
            tr.onclick = () => ask(word, text, sc.within);
            tr.onkeydown = (e) => {
              if (e.key === "Enter") ask(word, text, sc.within);
            };
            return tr;
          }),
        ),
      );
    };

    const drawNet = () => {
      netToggle.setAttribute("aria-pressed", String(netOn));
      if (!netOn || data === null || data.rows.length === 0) {
        netBox.replaceChildren();
        return;
      }
      const net = network(other === null ? [{ word: asked, rows: data.rows }] : [{ word: asked, rows: data.rows }, { word: other.word, rows: other.data.rows }]);
      const NS = "http://www.w3.org/2000/svg";
      const el2 = <K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>, text?: string) => {
        const e = document.createElementNS(NS, tag);
        for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
        if (text !== undefined) e.textContent = text;
        return e;
      };
      const svg = el2("svg", { viewBox: "-1.75 -1.3 3.5 2.6", class: "co-net-svg", role: "img", "aria-label": `Collocation network of ${asked}` });
      const at = new Map(net.nodes.map((n) => [n.id, n]));
      for (const e of net.edges) {
        const a = at.get(e.from)!;
        const b = at.get(e.to)!;
        svg.append(el2("line", { x1: a.x, y1: a.y, x2: b.x, y2: b.y, class: "co-net-edge", "stroke-width": e.width * 0.006 }));
      }
      for (const n of net.nodes) {
        const g = el2("g", { class: `co-net-node co-net-${n.kind}`, tabindex: 0 });
        g.append(el2("circle", { cx: n.x, cy: n.y, r: n.kind === "centre" ? 0.06 : 0.025 + 0.02 * n.strength }));
        g.append(el2("text", { x: n.x, y: n.y - (n.kind === "centre" ? 0.09 : 0.05), "text-anchor": "middle" }, n.label));
        const open = () => {
          if (n.kind === "centre") {
            // The second word's centre: back to one word.
            if (n.id === "c1") {
              versus.value = "";
              void load();
            }
            return;
          }
          versus.value = n.label;
          void load();
        };
        g.addEventListener("click", open);
        g.addEventListener("keydown", (e) => {
          if ((e as KeyboardEvent).key === "Enter") open();
        });
        g.append(el2("title", {}, n.kind === "centre" ? (n.id === "c1" ? `${n.label}: click to go back to «${asked}» alone` : n.label) : `${n.label}: click to set it beside «${asked}» and see what they share`));
        svg.append(g);
      }
      const save = h("button", { type: "button", className: "link-button", textContent: "Save picture…", title: "Save the network as an SVG file for a paper or slides" });
      save.onclick = async () => {
        const folder = await pickFolder({ directory: true, title: "Where should the picture go?" });
        if (typeof folder !== "string") return;
        try {
          const out = await saveDocument({ folder, title: `${asked}${other === null ? "" : ` and ${other.word}`} collocates`, extension: "svg", contents: standalone(svg) });
          showToast({ tone: "info", lines: [`Saved the network as ${out.path}.`] });
        } catch (e) {
          showToast({ tone: "error", lines: [errorText(e)] });
        }
      };
      netBox.replaceChildren(svg, save, h("p", { className: "note", textContent: other === null ? "The 14 strongest collocates by logDice, stronger nearer, thicker lines for more times together. Click one to set it beside the word." : `Collocates «${asked}» and «${other.word}» share sit between them, each nearer the word it keeps closer company with.` }));
    };

    const drawRows = () => {
      drawNet();
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
            const open = () => ask(asked, text, sc.within);
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
    netToggle.onclick = () => {
      netOn = !netOn;
      try {
        localStorage.setItem("wugs.collocates.network", netOn ? "on" : "off");
      } catch {
        // Not kept: fine.
      }
      drawNet();
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
        h("div", { className: "cc-bar" }, sorter, copy, netToggle),
        netBox,
        note,
        h("div", { className: "fq-rows" }, table),
      );
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
