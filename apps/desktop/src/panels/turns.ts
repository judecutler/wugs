// Turn-taking: how quickly speakers take the floor from each other in the text
// on screen, with one transcription track per speaker. A histogram of floor
// transfer offsets, the summary in words and each direction's median. Rules in
// ../turns/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { comparable } from "../agreement/logic";
import { errorText, saveDocument } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { getState, showToast } from "../store";
import { BIN_MS, byPair, histogram, NEAR_MS, RANGE_MS, summarize, turns, turnsLines, turnsTsv } from "../turns/logic";
import type { PanelDef, PanelView } from "./panel";
import { standalone } from "./vowels";

const SVG = "http://www.w3.org/2000/svg";
const W = 560;
const H = 200;
const LEFT = 40;
const RIGHT = 12;
const TOP = 12;
const BOTTOM = 40;

function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>, text?: string): SVGElementTagNameMap[K] {
  const el = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  if (text !== undefined) el.textContent = text;
  return el;
}

const signed = (v: number) => `${v > 0 ? "+" : v < 0 ? "−" : ""}${Math.abs(Math.round(v))}`;

export const turnsPanel: PanelDef = {
  title: "Turn-taking",
  create(): PanelView {
    const el = h("div", { className: "panel panel-turns" });
    let left: Set<string> = new Set();
    let leftFor = "";

    const paint = () => {
      const s = getState();
      if (s.kind !== "open") {
        el.replaceChildren(emptyState("No project open", "Open a project to see how its speakers take turns."));
        return;
      }
      if (s.text === null) {
        el.replaceChildren(emptyState("Turn-taking", "Select a conversation transcribed with a track per speaker to see how quickly they take the floor from each other."));
        return;
      }
      const text = s.text;
      const tracks = comparable(text.tiers, text.segments);
      if (tracks.length < 2) {
        el.replaceChildren(emptyState("Turn-taking", "This text needs a track per speaker, each with timed segments. Add a track on the Timeline for each speaker, or import an ELAN file with a tier per speaker."));
        return;
      }
      if (leftFor !== text.id) {
        left = new Set();
        leftFor = text.id;
      }
      const used = tracks.filter((t) => !left.has(t.id)).map((t) => t.id);
      const nameOf = (id: string) => text.tiers.find((t) => t.id === id)?.name ?? id;
      const t = turns(text.segments, used);
      const sum = summarize(t.transitions);

      const boxes = tracks.map((tr) => {
        const box = h("input", { type: "checkbox", checked: !left.has(tr.id) });
        box.onchange = () => {
          if (box.checked) left.delete(tr.id);
          else left.add(tr.id);
          paint();
        };
        return h("label", { title: "Count this track as one speaker's turns" }, box, ` ${tr.name}`);
      });
      const copy = h("button", { textContent: "Copy every change", disabled: t.transitions.length === 0, title: "Each change of speaker with the times and offset, for R or a spreadsheet" });
      copy.onclick = () =>
        void writeText(turnsTsv(t.transitions, nameOf)).then(
          () => showToast({ tone: "info", lines: [`Copied ${t.transitions.length} changes of speaker.`] }),
          (e) => showToast({ tone: "error", lines: [errorText(e)] }),
        );
      const save = h("button", { textContent: "Save picture…", disabled: sum === null, title: "Save the histogram as an SVG file" });
      save.onclick = () => {
        const shown = el.querySelector<SVGSVGElement>(".tt-plot");
        if (shown === null) return;
        const contents = standalone(shown);
        void (async () => {
          const folder = await pickFile({ directory: true, title: "Where should the picture go?" });
          if (typeof folder !== "string") return;
          try {
            const r = await saveDocument({ folder, title: `${text.title} turn-taking`, extension: "svg", contents });
            showToast({ tone: "info", lines: [`Saved the picture as ${r.path}.`] });
          } catch (e) {
            showToast({ tone: "error", lines: [errorText(e)] });
          }
        })();
      };
      const body: Node[] = [h("div", { className: "vw-bar" }, ...boxes), h("div", { className: "vw-bar" }, copy, save), h("div", { className: "ag-summary" }, ...turnsLines(t, sum).map((line) => h("p", { textContent: line })))];

      if (sum !== null) {
        const bins = histogram(t.transitions);
        const max = Math.max(1, ...bins.map((b) => b.count));
        const x = (ms: number) => LEFT + ((ms + RANGE_MS) / (2 * RANGE_MS)) * (W - LEFT - RIGHT);
        const y = (n: number) => TOP + (1 - n / max) * (H - TOP - BOTTOM);
        const root = svg("svg", { viewBox: `0 0 ${W} ${H}`, class: "vw-plot tt-plot", role: "img", "aria-label": `Floor transfer offsets: ${sum.n} changes of speaker, median ${Math.round(sum.median)} ms` });
        root.append(svg("rect", { x: x(-NEAR_MS), y: TOP, width: x(NEAR_MS) - x(-NEAR_MS), height: H - TOP - BOTTOM, class: "tt-near" }));
        for (const b of bins) {
          if (b.count === 0) continue;
          const bar = svg("rect", { x: x(b.from) + 1, y: y(b.count), width: x(b.from + BIN_MS) - x(b.from) - 2, height: H - BOTTOM - y(b.count), class: b.from < 0 ? "tt-bar tt-overlap" : "tt-bar" });
          bar.append(svg("title", {}, `${b.count} from ${signed(b.from)} to ${signed(b.from + BIN_MS)} ms`));
          root.append(bar);
        }
        for (const tick of [-2000, -1000, 0, 1000, 2000]) {
          root.append(svg("line", { x1: x(tick), x2: x(tick), y1: H - BOTTOM, y2: H - BOTTOM + 5, class: "vw-grid" }), svg("text", { x: x(tick), y: H - BOTTOM + 18, class: "vw-tick", "text-anchor": "middle" }, tick === 0 ? "0" : `${signed(tick / 1000)} s`));
        }
        root.append(
          svg("line", { x1: x(0), x2: x(0), y1: TOP, y2: H - BOTTOM, class: "tt-zero" }),
          svg("line", { x1: x(Math.max(-RANGE_MS, Math.min(RANGE_MS, sum.median))), x2: x(Math.max(-RANGE_MS, Math.min(RANGE_MS, sum.median))), y1: TOP, y2: H - BOTTOM, class: "tt-median" }),
          svg("text", { x: x(-RANGE_MS / 2), y: H - 4, class: "vw-axis", "text-anchor": "middle" }, "overlap"),
          svg("text", { x: x(RANGE_MS / 2), y: H - 4, class: "vw-axis", "text-anchor": "middle" }, "gap"),
          svg("text", { x: LEFT - 6, y: TOP + 10, class: "vw-tick", "text-anchor": "end" }, String(max)),
          svg("text", { x: LEFT - 6, y: H - BOTTOM, class: "vw-tick", "text-anchor": "end" }, "0"),
        );
        body.push(root, h("p", { className: "note", textContent: `Bars to the left of zero overlap the previous turn, to the right leave a gap; the shaded band is ±${NEAR_MS} ms and the dark line the median. Offsets past ±2 s sit in the end bars.` }));
        const pairs = byPair(t.transitions);
        body.push(
          h(
            "div",
            { className: "ag-scroll" },
            h(
            "table",
            { className: "pg-table" },
            h("thead", {}, h("tr", {}, ...["From", "To", "Changes", "Median (ms)", "Mean (ms)"].map((c) => h("th", { scope: "col", textContent: c })))),
            h(
              "tbody",
              {},
              ...pairs.map((p) =>
                h(
                  "tr",
                  {},
                  h("td", { textContent: nameOf(p.from) }),
                  h("td", { textContent: nameOf(p.to) }),
                  h("td", { className: "fq-num", textContent: String(p.summary.n) }),
                  h("td", { className: "fq-num", textContent: signed(p.summary.median) }),
                  h("td", { className: "fq-num", textContent: signed(p.summary.mean) }),
                ),
              ),
            ),
            ),
          ),
        );
      }
      el.replaceChildren(...body);
    };

    const unprovide = provide("turns", () => [
      { id: "turns:copy", title: "Copy the changes of speaker", group: "Turn-taking", words: ["conversation", "floor transfer", "gap", "overlap", "turn-taking", "speakers"], run: () => el.querySelector<HTMLButtonElement>(".vw-bar button")?.click() },
    ]);
    return { element: el, render: paint, dispose: unprovide };
  },
};
