// Pitch contours: each timed segment's F0 at ten points spread evenly
// through it, drawn as a line from the segment's start to its end, and each
// label's mean contour on top, in Hz or semitones from the speaker's median.
// Label the segments by tone or intonation pattern (H, L, HL, rise…) and the
// categories show as shapes. The numbers come from the acoustic-analysis
// capability (src-tauri/src/measure.rs), as Measure… writes them. Rules in
// ../contours/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { errorText, measurePoints, saveDocument } from "../api";
import { contourNote, contourTable, dashOf, lines, means, POSITIONS, reference, valueRange, type ContourPoint, type Unit } from "../contours/logic";
import { emptyState, h } from "../dom";
import { hear, recordingForShown } from "../hear/hear";
import { trackToMeasure } from "../measure/logic";
import { provide } from "../palette/registry";
import { getSelection, getState, showToast } from "../store";
import { ticks } from "../vowels/logic";
import type { PanelDef, PanelView } from "./panel";
import { standalone } from "./vowels";

const SVG = "http://www.w3.org/2000/svg";
const UNIT_KEY = "wugs.contours.unit";
const TOKENS_KEY = "wugs.contours.tokens";
const W = 560;
const H = 380;
const BOX = { left: 56, top: 16, width: W - 140, height: H - 60 };

function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>, text?: string): SVGElementTagNameMap[K] {
  const el = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  if (text !== undefined) el.textContent = text;
  return el;
}

/** Draw values as one path, broken where the voice is off. */
function pathOf(values: (number | null)[], x: (i: number) => number, y: (v: number) => number): string {
  let d = "";
  let pen = false;
  values.forEach((v, i) => {
    if (v === null) {
      pen = false;
      return;
    }
    d += `${pen ? "L" : "M"}${x(i).toFixed(1)},${y(v).toFixed(1)}`;
    pen = true;
  });
  return d;
}

export const contoursPanel: PanelDef = {
  title: "Pitch contours",
  create(): PanelView {
    const el = h("div", { className: "panel panel-contours" });
    let unit: Unit = localStorage.getItem(UNIT_KEY) === "hz" ? "hz" : "st";
    let showTokens = localStorage.getItem(TOKENS_KEY) !== "off";
    let tier: string | null = null;
    let result: { key: string; textId: string; tierId: string; track: string; points: ContourPoint[]; untimed: number; analysedBy: string } | null = null;
    let busy = false;
    let problem: string | null = null;
    let lastText = "";

    const run = async () => {
      const s = getState();
      if (s.kind !== "open" || s.text === null || tier === null || busy) return;
      const source = recordingForShown();
      if (source === null) {
        problem = "Load this text's recording on the Timeline first.";
        paint();
        return;
      }
      const textId = s.text.id;
      const tierId = tier;
      const track = s.text.tiers.find((t) => t.id === tierId)?.name ?? "";
      busy = true;
      problem = null;
      paint();
      try {
        const r = await measurePoints({ textId, tierId, source });
        result = { key: `${textId}|${tierId}`, textId, tierId, track, points: r.points, untimed: r.untimed, analysedBy: r.analysedBy };
      } catch (e) {
        problem = errorText(e);
      }
      busy = false;
      paint();
    };

    const plot = (): SVGSVGElement => {
      const r = result!;
      const shown = lines(r.points, unit);
      const ms = means(shown);
      const rg = valueRange(shown, unit);
      const x = (i: number) => BOX.left + (POSITIONS[i]! / 100) * BOX.width;
      const y = (v: number) => BOX.top + ((rg.hi - v) / (rg.hi - rg.lo)) * BOX.height;
      const root = svg("svg", { viewBox: `0 0 ${W} ${H}`, class: "vw-plot ct-plot", role: "img", "aria-label": `Pitch contours of ${r.track}: ${ms.length} labels` });
      for (const t of ticks(rg)) {
        root.append(svg("line", { x1: BOX.left, x2: BOX.left + BOX.width, y1: y(t), y2: y(t), class: "vw-grid" }), svg("text", { x: BOX.left - 6, y: y(t) + 4, class: "vw-tick", "text-anchor": "end" }, String(t)));
      }
      for (const pct of [0, 25, 50, 75, 100]) {
        const px = BOX.left + (pct / 100) * BOX.width;
        root.append(svg("text", { x: px, y: BOX.top + BOX.height + 16, class: "vw-tick", "text-anchor": "middle" }, `${pct}%`));
      }
      root.append(
        svg("rect", { x: BOX.left, y: BOX.top, width: BOX.width, height: BOX.height, class: "ct-frame" }),
        svg("text", { x: BOX.left + BOX.width / 2, y: H - 8, class: "vw-axis", "text-anchor": "middle" }, "Time through the segment"),
        svg("text", { x: 14, y: BOX.top + BOX.height / 2, class: "vw-axis", "text-anchor": "middle", transform: `rotate(-90 14 ${BOX.top + BOX.height / 2})` }, unit === "st" ? "F0 (semitones)" : "F0 (Hz)"),
      );
      if (showTokens) {
        for (const l of shown) {
          const d = pathOf(l.values, x, y);
          if (d === "") continue;
          const line = svg("path", { d, class: "ct-token", tabindex: 0 });
          line.append(svg("title", {}, `${l.group} — click to hear it`));
          const open = () => void hear({ textId: r.textId, tierId: r.tierId, segmentId: l.point.segment, startMs: l.point.startMs, endMs: l.point.endMs });
          line.addEventListener("click", open);
          line.addEventListener("keydown", (ev) => {
            if ((ev as KeyboardEvent).key === "Enter") open();
          });
          if (getSelection()?.segmentId === l.point.segment) line.classList.add("is-selected");
          root.append(line);
        }
      }
      // Each mean labelled at its right end, so no legend is needed; labels that
      // would overlap are pushed apart.
      const ends: { label: string; y: number }[] = [];
      ms.forEach((m, n) => {
        const d = pathOf(m.values, x, y);
        if (d === "") return;
        root.append(svg("path", { d, class: "ct-mean", "stroke-dasharray": dashOf(n) }));
        const last = [...m.values].reverse().find((v) => v !== null);
        if (last != null) ends.push({ label: `${m.label.length > 10 ? `${m.label.slice(0, 9)}…` : m.label} (${m.count})`, y: y(last) });
      });
      ends.sort((a, b) => a.y - b.y);
      let floor = -Infinity;
      for (const e of ends) {
        const at = Math.max(e.y, floor + 16);
        floor = at;
        root.append(svg("text", { x: BOX.left + BOX.width + 8, y: at + 5, class: "ct-label" }, e.label));
      }
      return root;
    };

    const paint = () => {
      const s = getState();
      if (s.kind !== "open") {
        el.replaceChildren(emptyState("No project open", "Open a project to draw its pitch contours."));
        return;
      }
      if (s.text === null) {
        el.replaceChildren(emptyState("Pitch contours", "Select a text whose recording is segmented, one syllable, word or phrase to a segment, to draw how its pitch moves."));
        return;
      }
      const timedTiers = s.text.tiers.filter((t) => s.text!.segments.some((x) => x.tier === t.id && x.startMs !== null && x.endMs !== null && x.endMs > x.startMs));
      if (tier === null || !timedTiers.some((t) => t.id === tier)) tier = trackToMeasure(s.text.tiers, s.text.segments, getSelection()?.segmentId ?? null)?.id ?? null;
      if (timedTiers.length === 0) {
        el.replaceChildren(emptyState("Pitch contours", "This text has no timed segments yet. Segment the recording on the Timeline, then draw the contours here."));
        return;
      }
      const picker = h("select", { title: "The track to draw: each timed segment is one line, grouped by its label" }, ...timedTiers.map((t) => h("option", { value: t.id, textContent: t.name })));
      picker.value = tier ?? "";
      picker.onchange = () => {
        tier = picker.value;
        paint();
      };
      const go = h("button", { className: "primary", textContent: busy ? "Measuring…" : "Draw", disabled: busy, title: "Measure the pitch of every timed segment of this track at ten points and draw it" });
      go.onclick = () => void run();
      const units = h("select", { title: "Semitones from the speaker's median compare speakers with different voices; Hz is the raw frequency" }, h("option", { value: "st", textContent: "Semitones" }), h("option", { value: "hz", textContent: "Hz" }));
      units.value = unit;
      units.onchange = () => {
        unit = units.value === "hz" ? "hz" : "st";
        localStorage.setItem(UNIT_KEY, unit);
        paint();
      };
      const tok = h("input", { type: "checkbox", checked: showTokens });
      tok.onchange = () => {
        showTokens = tok.checked;
        localStorage.setItem(TOKENS_KEY, showTokens ? "on" : "off");
        paint();
      };
      const current = result !== null && result.key === `${s.text.id}|${tier}` ? result : null;
      const copy = h("button", { textContent: "Copy table", disabled: current === null, title: "Copy every segment's label, times and F0 at the ten points (Hz) as a table for a spreadsheet or R" });
      copy.onclick = () => {
        if (current === null) return;
        void writeText(contourTable(current.points)).then(
          () => showToast({ tone: "info", lines: [`Copied ${current.points.length} rows.`] }),
          (e) => showToast({ tone: "error", lines: [errorText(e)] }),
        );
      };
      const save = h("button", { textContent: "Save picture…", disabled: current === null, title: "Save the plot as an SVG file for a paper or slides" });
      save.onclick = () => {
        const shownPlot = el.querySelector<SVGSVGElement>(".ct-plot");
        if (current === null || shownPlot === null) return;
        const contents = standalone(shownPlot);
        void (async () => {
          const folder = await pickFile({ directory: true, title: "Where should the picture go?" });
          if (typeof folder !== "string") return;
          try {
            const r = await saveDocument({ folder, title: `${current.track} pitch contours`, extension: "svg", contents });
            showToast({ tone: "info", lines: [`Saved the picture as ${r.path}.`] });
          } catch (e) {
            showToast({ tone: "error", lines: [errorText(e)] });
          }
        })();
      };
      const bar = h("div", { className: "vw-bar" }, h("label", {}, "Track ", picker), go, h("label", {}, "Units ", units), h("label", { title: "Draw a faint line for every segment, not only each label's mean" }, tok, " Every token"), copy, save);
      const body: Node[] = [];
      if (problem !== null) body.push(h("p", { className: "note vw-problem", textContent: problem }));
      if (current !== null) {
        const drawn = lines(current.points, unit).length;
        body.push(plot(), h("p", { className: "note", textContent: contourNote(current.points, drawn, current.untimed, current.analysedBy, unit, reference(current.points)) }));
      } else if (problem === null) {
        body.push(
          h("p", {
            className: "note",
            textContent: "Draw measures the pitch of every timed segment of the track at ten points from its start to its end, so short and long segments line up. Label segments with their tone or tune (H, L, HL, rise…) and each label gets its mean contour.",
          }),
        );
      }
      el.replaceChildren(bar, ...body);
    };

    const render = () => {
      const s = getState();
      const now = s.kind === "open" ? `${s.info.projectId}|${s.text?.id ?? ""}|${s.text?.segments.length ?? 0}` : "closed";
      if (now !== lastText) {
        lastText = now;
        problem = null;
      }
      paint();
    };

    const unprovide = provide("contours", () => [{ id: "contours:draw", title: "Draw pitch contours", group: "Pitch contours", words: ["tone", "intonation", "f0", "pitch", "phonetics", "time-normalized"], run: () => void run() }]);
    return { element: el, render, dispose: unprovide };
  },
};
