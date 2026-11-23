// Sentence (Scrolls, SCROLLS.md §1 "Correct where you look"): the selected
// sentence's dependency tree drawn as arcs over its words, and a table of
// lemma, tags, features, head and relation to correct in place. Each change
// is one write of the sentence's annotation (wugs_conllu::tree::set_cell).
import { errorText, sentenceGet, sentenceSet } from "../api";
import { emptyState, h } from "../dom";
import { arcs, columns, DEPRELS, headChoices, rootOf, UPOS, type SentenceView, type WordRow } from "../sentence/logic";
import { dataGeneration, getSelection, getState, showToast, subscribeSelection } from "../store";
import type { PanelDef, PanelView } from "./panel";
import { tagBar } from "../morphosyntax/bar";

const SVG = "http://www.w3.org/2000/svg";
const LEVEL_PX = 26;
const WORD_Y_PAD = 18;

function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>, text?: string): SVGElementTagNameMap[K] {
  const el = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  if (text !== undefined) el.textContent = text;
  return el;
}

/** The tree: words along the bottom with their UPOS, arcs from head to dependent above. */
function tree(view: SentenceView, picked: number | null, pick: (i: number) => void): SVGSVGElement {
  const cols = columns(view.words, 9, 18, 40);
  const all = arcs(view.words);
  const top = Math.max(1, ...all.map((a) => a.level)) * LEVEL_PX + 24;
  const width = cols.length === 0 ? 0 : cols[cols.length - 1]!.x + cols[cols.length - 1]!.width;
  const height = top + WORD_Y_PAD + 30;
  const root = rootOf(view.words);
  const out = svg("svg", { class: "st-tree", width, height, viewBox: `0 0 ${width} ${height}`, role: "img" });
  out.setAttribute("aria-label", `Dependency tree of «${view.content}»`);
  const mid = (i: number) => cols[i]!.x + cols[i]!.width / 2;
  for (const a of all) {
    // Arcs leave the head a little right of centre and reach the dependent
    // a little left, so two arcs at one word stay apart.
    const x1 = mid(a.head) + (a.dep > a.head ? 4 : -4);
    const x2 = mid(a.dep) + (a.dep > a.head ? -4 : 4);
    const y = top;
    const peak = top - a.level * LEVEL_PX;
    const path = svg("path", { d: `M ${x1} ${y} C ${x1} ${peak}, ${x2} ${peak}, ${x2} ${y}`, class: `st-arc${picked === a.dep || picked === a.head ? " st-picked" : ""}` });
    out.append(path, svg("path", { d: `M ${x2 - 4} ${y - 6} L ${x2} ${y} L ${x2 + 4} ${y - 6}`, class: "st-arrow" }));
    // A curve whose control points both sit at `peak` tops out three quarters of the way up; the label sits just above.
    const apex = y - 0.75 * (y - peak);
    out.append(svg("text", { x: (x1 + x2) / 2, y: apex - 4, class: "st-rel", "text-anchor": "middle" }, a.label || "·"));
  }
  if (root !== null) {
    const x = mid(root);
    out.append(svg("line", { x1: x, y1: 14, x2: x, y2: top, class: "st-arc st-root" }), svg("text", { x: x + 4, y: 12, class: "st-rel" }, "root"));
  }
  view.words.forEach((w, i) => {
    const g = svg("g", { class: `st-word${picked === i ? " st-picked" : ""}`, tabindex: 0, role: "button" });
    g.setAttribute("aria-label", `Word ${i + 1}, ${w.form}`);
    g.append(svg("text", { x: mid(i), y: top + WORD_Y_PAD, "text-anchor": "middle", class: "st-form" }, w.form), svg("text", { x: mid(i), y: top + WORD_Y_PAD + 16, "text-anchor": "middle", class: "st-upos" }, w.upos || "_"));
    g.addEventListener("click", () => pick(i));
    g.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        pick(i);
      }
    });
    out.append(g);
  });
  return out;
}

export const sentencePanel: PanelDef = {
  title: "Sentence",
  create(): PanelView {
    const el = h("div", { className: "panel panel-sentence" });
    let view: SentenceView | null = null;
    let viewText = "";
    let key = "";
    let generation = -1;
    let seq = 0;
    let picked: number | null = null;
    let queue: Promise<unknown> = Promise.resolve();
    const relations = h("datalist", { id: "st-deprels" }, ...DEPRELS.map((d) => h("option", { value: d })));
    /** The Tag and parse bar for the text on screen, redrawn in place when its data arrives. */
    const bar = (): HTMLElement => {
      const s = getState();
      if (s.kind !== "open" || s.text === null) return h("span");
      return tagBar({ id: s.text.id, title: s.text.title }, () => {
        const old = el.querySelector(".st-tag");
        if (old !== null) old.replaceWith(bar());
      });
    };

    const selected = () => {
      const s = getState();
      const sel = getSelection();
      if (s.kind !== "open" || s.text === null || sel === null || sel.textId !== s.text.id) return null;
      const segment = s.text.segments.find((x) => x.id === sel.segmentId);
      return segment === undefined ? null : { text: s.text, segment };
    };

    const load = (textId: string, segmentId: string) => {
      const mine = ++seq;
      key = `${textId}|${segmentId}`;
      generation = dataGeneration();
      void sentenceGet({ textId, segmentId }).then(
        (v) => {
          if (mine !== seq) return;
          view = v;
          viewText = textId;
          picked = null;
          paint();
        },
        (e) => {
          if (mine === seq) el.replaceChildren(emptyState("Sentence", errorText(e)));
        },
      );
    };

    /** Corrections go one after another; the sentence that comes back last is the newest. */
    const correct = (w: WordRow, column: string, value: string) => {
      if (view === null) return;
      const at = { textId: viewText, segmentId: view.segmentId };
      queue = queue.then(async () => {
        try {
          const v = await sentenceSet({ ...at, tokenId: w.token, column, value });
          if (view !== null && v.segmentId === view.segmentId) view = v;
        } catch (e) {
          showToast({ tone: "error", lines: [errorText(e)] });
        }
        paint();
      });
    };

    const cell = (w: WordRow, column: "lemma" | "xpos" | "feats" | "deprel", label: string, wide = false) => {
      const input = h("input", { value: w[column], spellcheck: false, className: wide ? "st-wide" : "" });
      input.setAttribute("aria-label", `${label} of ${w.form}`);
      if (column === "deprel") input.setAttribute("list", "st-deprels");
      input.onchange = () => correct(w, column, input.value);
      return h("td", {}, input);
    };

    const table = (v: SentenceView) => {
      const head = h("tr", {}, ...["#", "Form", "Lemma", "UPOS", "XPOS", "Features", "Head", "Relation"].map((t) => h("th", { textContent: t })));
      const rows = v.words.map((w, i) => {
        const upos = h("select", {}, h("option", { value: "", textContent: "_" }), ...UPOS.map((u) => h("option", { value: u, textContent: u, selected: u === w.upos })));
        upos.setAttribute("aria-label", `UPOS of ${w.form}`);
        upos.onchange = () => correct(w, "upos", upos.value);
        const headSel = h("select", {}, ...headChoices(v.words, i).map((c) => h("option", { value: c.value, textContent: c.label, selected: c.value === (w.head === null ? "" : String(w.head)) })));
        headSel.setAttribute("aria-label", `Head of ${w.form}`);
        headSel.onchange = () => correct(w, "head", headSel.value);
        const tr = h(
          "tr",
          { className: picked === i ? "st-picked" : "" },
          h("td", { className: "st-num", textContent: String(i + 1) }),
          h("td", { className: "st-form-cell", textContent: w.form }),
          cell(w, "lemma", "Lemma"),
          h("td", {}, upos),
          cell(w, "xpos", "XPOS"),
          cell(w, "feats", "Features", true),
          h("td", {}, headSel),
          cell(w, "deprel", "Relation"),
        );
        tr.onclick = () => {
          if (picked !== i) {
            picked = i;
            for (const [j, row] of [...table.querySelectorAll("tr")].slice(1).entries()) row.classList.toggle("st-picked", j === i);
          }
        };
        return tr;
      });
      const table = h("table", { className: "fq-table st-table" }, h("thead", {}, head), h("tbody", {}, ...rows));
      return table;
    };

    function paint(): void {
      if (view === null) return;
      const v = view;
      const notes: HTMLElement[] = [];
      if (!v.annotated && v.words.length > 0) notes.push(h("p", { className: "note", textContent: "No tags or tree yet. Tag and parse the text above, or choose a head and relation for each word below." }));
      if (v.emptyNodes > 0) notes.push(h("p", { className: "note", textContent: `${v.emptyNodes} empty ${v.emptyNodes === 1 ? "node is" : "nodes are"} kept as imported and not drawn.` }));
      for (const p of v.problems) notes.push(h("p", { className: "note st-problem", textContent: p }));
      if (v.words.length === 0) {
        el.replaceChildren(bar(), emptyState("Sentence", "This segment is not broken into words yet. Tag and parse the text above, break it into words in the Gloss panel, or import a tagged text (CoNLL-U or TEI)."));
        return;
      }
      el.replaceChildren(
        bar(),
        h("p", { className: "st-content", textContent: v.content }),
        h("div", { className: "st-tree-box" }, tree(v, picked, (i) => {
          picked = i;
          paint();
          el.querySelectorAll<HTMLInputElement>(".st-table tbody tr")[i]?.querySelector<HTMLInputElement>("input")?.focus();
        })),
        ...notes,
        h("div", { className: "st-table-box" }, table(v)),
        relations,
      );
    }

    const draw = () => {
      const s = getState();
      if (s.kind === "closed") {
        view = null;
        key = "";
        el.replaceChildren(emptyState("No project open", "Open a project to see its sentences' trees."));
        return;
      }
      const sel = selected();
      if (sel === null) {
        view = null;
        key = "";
        el.replaceChildren(bar(), emptyState("Sentence", "Select a sentence in Transcribe, the Concordance or Search. Its words, tags and dependency tree appear here, ready to correct."));
        return;
      }
      const wanted = `${sel.text.id}|${sel.segment.id}`;
      // A redraw while a cell is being typed in waits for the change to save.
      if (el.contains(document.activeElement) && document.activeElement instanceof HTMLInputElement && wanted === key) return;
      if (wanted !== key || generation !== dataGeneration()) load(sel.text.id, sel.segment.id);
    };

    const unsubscribe = subscribeSelection(draw);
    return {
      element: el,
      render: draw,
      dispose: unsubscribe,
    };
  },
};
