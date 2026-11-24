// Durations: how long each label's segments last on one track of the text on
// screen, as a strip plot (a dot per segment, the mean and one standard
// deviation as a bar), a table, and a comparison of two labels (how many
// times as long, Welch's t-test). Read from the segments' own times, so it
// needs no recording or analysis. Rules in ../durations/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { errorText, saveDocument } from "../api";
import { emptyState, h } from "../dom";
import { atLeast, cannotCompare, compare, compareLine, defaultPair, durationGroups, durationsNote, msTicks, summaryTsv, tokensTsv, type Group } from "../durations/logic";
import { rateLines, speakingRate } from "../durations/rate";
import { rhythm, rhythmLines, rhythmTsv } from "../durations/rhythm";
import { hear } from "../hear/hear";
import { trackToMeasure } from "../measure/logic";
import { provide } from "../palette/registry";
import { getSelection, getState, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";
import { standalone } from "./vowels";

const SVG = "http://www.w3.org/2000/svg";
const MIN_KEY = "wugs.durations.min";
const W = 560;
const ROW = 34;
const LEFT = 130;
const RIGHT = 20;
const MAX_ROWS = 30;

function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>, text?: string): SVGElementTagNameMap[K] {
  const el = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  if (text !== undefined) el.textContent = text;
  return el;
}

export const durationsPanel: PanelDef = {
  title: "Durations",
  create(): PanelView {
    const el = h("div", { className: "panel panel-durations" });
    let tier: string | null = null;
    let min = Number(localStorage.getItem(MIN_KEY) ?? "1") || 1;
    let pair: [string, string] | null = null;
    let pairFor = "";

    const plot = (shown: readonly Group[], textId: string, tierId: string, track: string): SVGSVGElement => {
      const max = Math.max(...shown.map((g) => g.max)) * 1.05;
      const height = 30 + shown.length * ROW + 34;
      const x = (ms: number) => LEFT + (ms / max) * (W - LEFT - RIGHT);
      const root = svg("svg", { viewBox: `0 0 ${W} ${height}`, class: "vw-plot du-plot", role: "img", "aria-label": `Durations of ${track}: ${shown.length} labels` });
      const bottom = 20 + shown.length * ROW;
      for (const t of msTicks(max)) {
        root.append(svg("line", { x1: x(t), x2: x(t), y1: 14, y2: bottom, class: "vw-grid" }), svg("text", { x: x(t), y: bottom + 16, class: "vw-tick", "text-anchor": "middle" }, String(t)));
      }
      root.append(svg("text", { x: LEFT + (W - LEFT - RIGHT) / 2, y: height - 6, class: "vw-axis", "text-anchor": "middle" }, "Duration (ms)"));
      const selected = getSelection()?.segmentId ?? null;
      shown.forEach((g, i) => {
        const cy = 20 + i * ROW + ROW / 2;
        root.append(svg("text", { x: LEFT - 10, y: cy + 5, class: "du-label", "text-anchor": "end" }, g.label.length > 12 ? `${g.label.slice(0, 11)}…` : g.label));
        // Dots spread a little up and down by their order, so equal times stay visible.
        g.tokens.forEach((t, k) => {
          const dot = svg("circle", { cx: x(t.ms), cy: cy + ((k % 5) - 2) * 2.5, r: 3.5, class: selected === t.segment ? "du-dot is-selected" : "du-dot", tabindex: 0 });
          dot.append(svg("title", {}, `${g.label}: ${Math.round(t.ms)} ms. Click to hear it.`));
          const play = () => void hear({ textId, tierId, segmentId: t.segment, startMs: t.startMs, endMs: t.endMs });
          dot.addEventListener("click", play);
          dot.addEventListener("keydown", (ev) => {
            if ((ev as KeyboardEvent).key === "Enter") play();
          });
          root.append(dot);
        });
        const sd = g.sd ?? 0;
        root.append(
          svg("line", { x1: x(Math.max(0, g.mean - sd)), x2: x(g.mean + sd), y1: cy + 10, y2: cy + 10, class: "du-sd" }),
          svg("line", { x1: x(g.mean), x2: x(g.mean), y1: cy - 9, y2: cy + 13, class: "du-mean" }),
        );
      });
      return root;
    };

    const copy = (text: string, what: string) =>
      void writeText(text).then(
        () => showToast({ tone: "info", lines: [`Copied ${what}.`] }),
        (e) => showToast({ tone: "error", lines: [errorText(e)] }),
      );

    const paint = () => {
      const s = getState();
      if (s.kind !== "open") {
        el.replaceChildren(emptyState("No project open", "Open a project to see how long its segments last."));
        return;
      }
      if (s.text === null) {
        el.replaceChildren(emptyState("Durations", "Select a text whose segments are labelled (a, aː, t, tː…) to compare how long each label lasts."));
        return;
      }
      const text = s.text;
      const timedTiers = text.tiers.filter((t) => text.segments.some((x) => x.tier === t.id && x.startMs !== null && x.endMs !== null && x.endMs > x.startMs && x.content.trim() !== ""));
      if (timedTiers.length === 0) {
        el.replaceChildren(emptyState("Durations", "This text has no labelled timed segments yet. Segment the recording on the Timeline and type a label in each."));
        return;
      }
      if (tier === null || !timedTiers.some((t) => t.id === tier)) tier = trackToMeasure(text.tiers, text.segments, getSelection()?.segmentId ?? null)?.id ?? timedTiers[0]!.id;
      if (!timedTiers.some((t) => t.id === tier)) tier = timedTiers[0]!.id;
      const tierId = tier;
      const track = text.tiers.find((t) => t.id === tierId)?.name ?? "";
      const { groups, untimed } = durationGroups(text.segments, tierId);
      const shown = atLeast(groups, min).slice(0, MAX_ROWS);
      const key = `${text.id}|${tierId}`;
      if (pairFor !== key || pair === null || !groups.some((g) => g.label === pair![0]) || !groups.some((g) => g.label === pair![1])) {
        pair = defaultPair(groups);
        pairFor = key;
      }

      const picker = h("select", { title: "The track whose segments to read: each label is one row" }, ...timedTiers.map((t) => h("option", { value: t.id, textContent: t.name })));
      picker.value = tierId;
      picker.onchange = () => {
        tier = picker.value;
        paint();
      };
      const minPick = h("select", { title: "Plot only labels with at least this many segments" }, ...[1, 2, 3, 5, 10].map((n) => h("option", { value: String(n), textContent: n === 1 ? "Every label" : `${n}+ segments` })));
      minPick.value = String(min);
      minPick.onchange = () => {
        min = Number(minPick.value) || 1;
        localStorage.setItem(MIN_KEY, String(min));
        paint();
      };
      const copySummary = h("button", { textContent: "Copy table", disabled: groups.length === 0, title: "Each label's count, mean, standard deviation, median, shortest and longest, for a spreadsheet" });
      copySummary.onclick = () => copy(summaryTsv(groups), `${groups.length} labels`);
      const copyTokens = h("button", { textContent: "Copy every segment", disabled: groups.length === 0, title: "Every segment's label, times and duration, one row each, for R or a spreadsheet" });
      copyTokens.onclick = () => copy(tokensTsv(groups), `${groups.reduce((n, g) => n + g.n, 0)} segments`);
      const save = h("button", { textContent: "Save picture…", disabled: shown.length === 0, title: "Save the plot as an SVG file for a paper or slides" });
      save.onclick = () => {
        const shownPlot = el.querySelector<SVGSVGElement>(".du-plot");
        if (shownPlot === null) return;
        const contents = standalone(shownPlot);
        void (async () => {
          const folder = await pickFile({ directory: true, title: "Where should the picture go?" });
          if (typeof folder !== "string") return;
          try {
            const r = await saveDocument({ folder, title: `${track} durations`, extension: "svg", contents });
            showToast({ tone: "info", lines: [`Saved the picture as ${r.path}.`] });
          } catch (e) {
            showToast({ tone: "error", lines: [errorText(e)] });
          }
        })();
      };
      const bar = h("div", { className: "vw-bar" }, h("label", {}, "Track ", picker), h("label", {}, "Show ", minPick), copySummary, copyTokens, save);

      const body: Node[] = [];
      if (shown.length > 0) body.push(plot(shown, text.id, tierId, track));
      body.push(h("p", { className: "note", textContent: durationsNote(groups, untimed, shown.length) }));
      if (groups.length > 0) {
        const head = ["Label", "n", "Mean", "SD", "Median", "Shortest", "Longest"];
        const ms = (v: number) => `${Math.round(v)}`;
        body.push(
          h(
            "table",
            { className: "pg-table du-table" },
            h("thead", {}, h("tr", {}, ...head.map((t) => h("th", { scope: "col", textContent: t })))),
            h(
              "tbody",
              {},
              ...groups.map((g) =>
                h(
                  "tr",
                  {},
                  h("th", { scope: "row", className: "du-label-cell", textContent: g.label }),
                  ...[String(g.n), ms(g.mean), g.sd === null ? "–" : ms(g.sd), ms(g.median), ms(g.min), ms(g.max)].map((v) => h("td", { className: "fq-num", textContent: v })),
                ),
              ),
            ),
          ),
        );
      }
      if (groups.length >= 2 && pair !== null) {
        const options = () => groups.map((g) => h("option", { value: g.label, textContent: `${g.label} (${g.n})` }));
        const aPick = h("select", { title: "The first label" }, ...options());
        const bPick = h("select", { title: "The label to compare with it" }, ...options());
        aPick.value = pair[0];
        bPick.value = pair[1];
        const change = () => {
          pair = [aPick.value, bPick.value];
          paint();
        };
        aPick.onchange = change;
        bPick.onchange = change;
        const a = groups.find((g) => g.label === pair![0]);
        const b = groups.find((g) => g.label === pair![1]);
        const c = a !== undefined && b !== undefined && a.label !== b.label ? compare(a, b) : null;
        body.push(
          h("h3", { textContent: "Compare two labels" }),
          h("div", { className: "vw-bar" }, aPick, h("span", { textContent: "with" }), bPick),
          h("p", { className: c === null ? "note" : "du-result", textContent: c === null ? cannotCompare(a, b) : compareLine(c) }),
        );
      }
      body.push(h("h3", { textContent: "Speaking rate" }), h("div", { className: "ag-summary du-rate" }, ...rateLines(speakingRate(text.segments, tierId)).map((line) => h("p", { textContent: line }))));
      // Rhythm, when the track holds one sound a segment (phones).
      const rh = rhythm(text.segments, tierId);
      const rhLines = rhythmLines(rh);
      if (rhLines.length > 0) {
        const copyIntervals = h("button", { type: "button", textContent: "Copy intervals", title: "Every vocalic and consonantal interval with its stretch and length, for a spreadsheet" });
        copyIntervals.onclick = () =>
          writeText(rhythmTsv(rh)).then(
            () => showToast({ tone: "info", lines: [`${rh.stretches.flat().length} intervals copied.`] }),
            (e) => showToast({ tone: "error", lines: [errorText(e)] }),
          );
        body.push(h("h3", { textContent: "Rhythm" }), h("div", { className: "ag-summary du-rhythm" }, ...rhLines.map((line) => h("p", { textContent: line }))), h("div", { className: "vw-bar" }, copyIntervals));
      }
      el.replaceChildren(bar, ...body);
    };

    const unprovide = provide("durations", () => [{ id: "durations:copy", title: "Copy segment durations", group: "Durations", words: ["length", "vowel length", "gemination", "duration", "phonetics", "t-test"], run: () => el.querySelector<HTMLButtonElement>(".vw-bar button")?.click() }]);
    return { element: el, render: paint, dispose: unprovide };
  },
};
