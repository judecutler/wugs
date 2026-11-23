// Interlinear: the whole selected text as interlinear glossed text, every
// sentence with its words, morphs, glosses and free translation, the way a
// FLEx or Toolbox user reads a text. Read-only: a sentence opens in the Gloss
// panel when clicked, ▶ plays it.
import { errorText, glossGet, type TextDetail } from "../api";
import { emptyState, h } from "../dom";
import { glossRuns, wordLines, type ExampleWord } from "../gloss/example";
import { numbering } from "../gloss/document";
import { translationLines } from "../gloss/translation";
import { hear, playButton } from "../hear/hear";
import { analysed, nextToGloss, progress, sentencesOf, toGloss, type Line } from "../interlinear/logic";
import { provide } from "../palette/registry";
import { dataGeneration, getSelection, getState, selectSegment, showToast, subscribeSelection } from "../store";
import type { PanelDef, PanelView } from "./panel";

/** Show only the sentences still to gloss (kept while the app runs). */
let onlyToGloss = false;

/** How many sentences are read from the engine at once. */
const BATCH = 8;

async function linesOf(text: TextDetail): Promise<Line[]> {
  // Numbered on their own track, as Copy example numbers them.
  const segs = text.segments.map((s) => ({ ...s, parent: s.parent ?? null }));
  const tiers = text.tiers.map((t) => ({ id: t.id, utterance: t.utterance === true }));
  const numbers = numbering(sentencesOf(segs, tiers, new Map()));
  const sentences = sentencesOf(segs, tiers, numbers);
  const lines: Line[] = [];
  for (let i = 0; i < sentences.length; i += BATCH) {
    const views = await Promise.all(sentences.slice(i, i + BATCH).map((seg) => glossGet({ textId: text.id, segmentId: seg.id })));
    views.forEach((v, j) => {
      const seg = sentences[i + j]!;
      const full = text.segments.find((s) => s.id === seg.id)!;
      const translation = translationLines(full, text.segments, text.tiers).find((l) => l.segment !== null)?.segment?.content ?? null;
      const words: ExampleWord[] = v.words.map((w) => ({ form: w.form, morphs: w.morphs.map((m) => ({ surface: m.surface, type: m.type, gloss: m.gloss })) }));
      lines.push({ id: seg.id, number: numbers.get(seg.id) ?? i + j + 1, content: seg.content, words, translation, startMs: seg.startMs });
    });
  }
  return lines;
}

function glossLine(gloss: string): HTMLElement {
  return h("span", { className: "il-gloss" }, ...glossRuns(gloss).map((r) => (r.caps ? h("span", { className: "sc", textContent: r.text }) : document.createTextNode(r.text))));
}

export const interlinearPanel: PanelDef = {
  title: "Interlinear",
  create(): PanelView {
    const el = h("div", { className: "panel panel-interlinear" });
    let key = "";
    let seq = 0;
    let lines: Line[] = [];

    /** Select the next sentence still to gloss, for the Gloss panel, and bring its row into view. */
    const goNext = () => {
      const s = getState();
      if (s.kind !== "open" || s.text === null) return;
      const text = s.text;
      const next = nextToGloss(lines, getSelection()?.segmentId ?? null);
      const seg = next === null ? undefined : text.segments.find((x) => x.id === next.id);
      if (seg === undefined) {
        showToast({ tone: "info", lines: [lines.length === 0 ? "This text has no sentences yet." : "Every sentence of this text is glossed."] });
        return;
      }
      selectSegment({ textId: text.id, tierId: seg.tier, segmentId: seg.id });
      window.requestAnimationFrame(() => el.querySelector<HTMLElement>(`.il-row[data-id="${CSS.escape(seg.id)}"]`)?.scrollIntoView({ block: "nearest" }));
    };

    const paint = (text: TextDetail) => {
      const sel = getSelection();
      const shown = onlyToGloss ? lines.filter(toGloss) : lines;
      const rows = shown.map((l) => {
        const seg = text.segments.find((s) => s.id === l.id);
        const play = playButton("Play this sentence", () => {
          if (seg !== undefined) void hear({ textId: text.id, tierId: seg.tier, segmentId: seg.id, startMs: seg.startMs, endMs: seg.endMs });
        });
        play.disabled = seg?.startMs === null || seg?.startMs === undefined;
        const body = analysed(l)
          ? h(
              "div",
              { className: "il-words" },
              ...l.words.map((w) => {
                const { morphs, glosses } = wordLines(w);
                return h("span", { className: "il-word" }, h("span", { className: "il-form", textContent: w.form }), h("span", { className: "il-morphs", textContent: w.morphs.length > 0 ? morphs : "" }), glossLine(glosses));
              }),
            )
          : h("p", { className: "il-plain", textContent: l.content.trim() === "" ? "(not transcribed)" : l.content });
        const row = h(
          "li",
          { className: `il-row${sel?.segmentId === l.id ? " il-selected" : ""}${analysed(l) ? "" : " il-unglossed"}`, tabIndex: 0, title: "Open this sentence in the Gloss panel" },
          h("span", { className: "il-num", textContent: `(${l.number})` }),
          play,
          h("div", { className: "il-body" }, body, l.translation === null ? null : h("p", { className: "il-tr", textContent: `‘${l.translation}’` })),
        );
        row.dataset.id = l.id;
        const pick = () => seg !== undefined && selectSegment({ textId: text.id, tierId: seg.tier, segmentId: seg.id });
        row.onclick = (e) => {
          if ((e.target as HTMLElement).closest("button") === null) pick();
        };
        row.onkeydown = (e) => {
          if (e.key === "Enter") pick();
          else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            e.preventDefault();
            const next = e.key === "ArrowDown" ? row.nextElementSibling : row.previousElementSibling;
            if (next instanceof HTMLElement) next.focus();
          }
        };
        return row;
      });
      const focused = el.contains(document.activeElement) ? [...el.querySelectorAll(".il-row")].indexOf(document.activeElement as Element) : -1;
      const only = h("input", { type: "checkbox", checked: onlyToGloss });
      only.onchange = () => {
        onlyToGloss = only.checked;
        paint(text);
      };
      const next = h("button", { type: "button", textContent: "Next to gloss", title: "Open the next sentence still to gloss in the Gloss panel" });
      next.onclick = goNext;
      const bar = h("div", { className: "il-bar" }, h("label", { className: "il-only" }, only, " Only sentences still to gloss"), next);
      const empty = onlyToGloss && shown.length === 0 && lines.length > 0 ? h("p", { className: "note", textContent: "Every sentence of this text is glossed." }) : null;
      el.replaceChildren(h("h3", { className: "il-title", textContent: text.title || "(untitled)" }), h("p", { className: "note", textContent: progress(lines) }), bar, ...(empty === null ? [] : [empty]), h("ol", { className: "il-list" }, ...rows));
      if (focused >= 0) el.querySelectorAll<HTMLElement>(".il-row")[focused]?.focus();
    };

    const draw = () => {
      const s = getState();
      if (s.kind !== "open") {
        key = "";
        el.replaceChildren(emptyState("No project open", "Open a project to read its texts interlinear."));
        return;
      }
      const text = s.text;
      if (text === null) {
        key = "";
        el.replaceChildren(emptyState("Interlinear", "Select a text in the Texts panel to read it here as interlinear glossed text, sentence by sentence."));
        return;
      }
      const wanted = `${text.id}|${dataGeneration()}|${text.segments.length}|${text.segments.map((x) => x.content).join("\u0001").length}`;
      if (wanted === key) {
        paint(text);
        return;
      }
      key = wanted;
      const mine = ++seq;
      if (lines.length === 0) el.replaceChildren(emptyState("Interlinear", "Reading the text…"));
      void linesOf(text).then(
        (ls) => {
          if (mine !== seq) return;
          lines = ls;
          const now = getState();
          if (now.kind === "open" && now.text?.id === text.id) paint(now.text);
        },
        (e) => {
          if (mine === seq) el.replaceChildren(emptyState("Interlinear", errorText(e)));
        },
      );
    };

    const unsubscribe = subscribeSelection(draw);
    const unprovide = provide("interlinear", () => [
      { id: "interlinear:next", title: "Go to the next sentence to gloss", group: "Interlinear", words: ["unglossed", "next", "skip", "todo"], run: goNext },
    ]);
    return {
      element: el,
      render: draw,
      dispose: () => {
        unsubscribe();
        unprovide();
      },
    };
  },
};
