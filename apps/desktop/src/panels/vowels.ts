// Vowel space: every timed segment of a track at its F1 and F2, the way a
// phonetician plots vowels (F2 right to left, F1 top to bottom), with each
// label's mean and a one-standard-deviation ellipse, in Hz or Lobanov-
// normalized. The numbers come from the acoustic-analysis capability
// (src-tauri/src/measure.rs), so they match the Timeline's overlays and
// Measure…'s table. Rules in ../vowels/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { errorText, measurePoints, saveDocument } from "../api";
import { emptyState, h } from "../dom";
import { hear, recordingForShown } from "../hear/hear";
import { trackToMeasure } from "../measure/logic";
import { provide } from "../palette/registry";
import { getSelection, getState, showToast } from "../store";
import { areaLine, hull, spansTheSpace, vowelArea, ellipsePath, groups, METHODS, parseMethod, place, plotNote, plotted, ranges, table, ticks, trajectories, wattCentroid, type Method, type Point } from "../vowels/logic";
import type { PanelDef, PanelView } from "./panel";

const SVG = "http://www.w3.org/2000/svg";
const NORMALIZE_KEY = "wugs.vowels.normalize";
const TOKENS_KEY = "wugs.vowels.tokens";
const PATHS_KEY = "wugs.vowels.trajectories";

function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>, text?: string): SVGElementTagNameMap[K] {
  const el = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  if (text !== undefined) el.textContent = text;
  return el;
}

/** The plot as a file of its own: every colour and font the stylesheet gave
 * it written onto the elements, so it looks the same in a paper or an editor. */
export function standalone(shown: SVGSVGElement): string {
  const copy = shown.cloneNode(true) as SVGSVGElement;
  const from = [shown, ...shown.querySelectorAll("*")];
  const to = [copy, ...copy.querySelectorAll("*")];
  const props = ["fill", "fill-opacity", "stroke", "stroke-width", "stroke-opacity", "stroke-dasharray", "font-family", "font-size", "font-style", "font-weight", "paint-order"];
  from.forEach((el, i) => {
    const target = to[i];
    if (!(el instanceof SVGElement) || target === undefined || el.tagName === "title") return;
    const cs = getComputedStyle(el);
    for (const p of props) {
      const v = cs.getPropertyValue(p);
      if (v !== "") target.setAttribute(p, v);
    }
    target.removeAttribute("class");
    target.removeAttribute("tabindex");
  });
  // The plot's own size, from its viewBox (every chart wugs saves draws in one).
  const [, , vw, vh] = (shown.getAttribute("viewBox") ?? `0 0 ${W} ${H}`).split(/\s+/).map(Number);
  const [width, height] = [vw ?? W, vh ?? H];
  copy.setAttribute("xmlns", SVG);
  copy.setAttribute("width", String(width));
  copy.setAttribute("height", String(height));
  const bg = document.createElementNS(SVG, "rect");
  for (const [k, v] of Object.entries({ x: 0, y: 0, width, height, fill: "#ffffff" })) bg.setAttribute(k, String(v));
  copy.insertBefore(bg, copy.firstChild);
  return `<?xml version="1.0" encoding="UTF-8"?>\n${new XMLSerializer().serializeToString(copy)}\n`;
}

const W = 560;
const H = 420;
const BOX = { left: 56, top: 16, width: W - 72, height: H - 60 };

export const vowelsPanel: PanelDef = {
  title: "Vowel space",
  create(): PanelView {
    const el = h("div", { className: "panel panel-vowels" });
    let normalize: Method = parseMethod(localStorage.getItem(NORMALIZE_KEY));
    let showTokens = localStorage.getItem(TOKENS_KEY) !== "off";
    let showPaths = localStorage.getItem(PATHS_KEY) === "on";
    let tier: string | null = null;
    type Measured = { key: string; textId: string; tierId: string; track: string; points: Point[]; untimed: number; analysedBy: string };
    let result: Measured | null = null;
    /** A second speaker's track to draw over the first, or none. */
    let other = "";
    let otherResult: Measured | null = null;
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
        result = { key: `${textId}|${tierId}`, textId, tierId, track, ...r };
        const otherId = other;
        otherResult = null;
        if (otherId !== "" && otherId !== tierId) {
          const r2 = await measurePoints({ textId, tierId: otherId, source });
          otherResult = { key: `${textId}|${otherId}`, textId, tierId: otherId, track: s.text.tiers.find((t) => t.id === otherId)?.name ?? "", ...r2 };
        }
      } catch (e) {
        problem = errorText(e);
      }
      busy = false;
      paint();
    };

    const plot = (): SVGSVGElement => {
      const r = result!;
      const ps = plotted(r.points, normalize);
      const gs = groups(ps);
      // A second speaker, each normalized against their own vowels.
      const o = otherResult !== null && otherResult.key === `${r.textId}|${other}` ? otherResult : null;
      const ps2 = o === null ? [] : plotted(o.points, normalize);
      const gs2 = groups(ps2);
      const rg = ranges([...ps, ...ps2], normalize);
      const root = svg("svg", { viewBox: `0 0 ${W} ${H}`, class: "vw-plot", role: "img", "aria-label": `Vowel space of ${r.track}: ${gs.length} labels` });
      // Grid and axes.
      for (const t of ticks(rg.x)) {
        const { px } = place(t, rg.y.lo, rg, BOX);
        root.append(svg("line", { x1: px, x2: px, y1: BOX.top, y2: BOX.top + BOX.height, class: "vw-grid" }), svg("text", { x: px, y: BOX.top + BOX.height + 16, class: "vw-tick", "text-anchor": "middle" }, String(t)));
      }
      for (const t of ticks(rg.y)) {
        const { py } = place(rg.x.hi, t, rg, BOX);
        root.append(svg("line", { x1: BOX.left, x2: BOX.left + BOX.width, y1: py, y2: py, class: "vw-grid" }), svg("text", { x: BOX.left - 6, y: py + 4, class: "vw-tick", "text-anchor": "end" }, String(t)));
      }
      const unit = ` (${METHODS.find((m) => m.id === normalize)?.unit ?? "Hz"})`;
      root.append(
        svg("text", { x: BOX.left + BOX.width / 2, y: H - 8, class: "vw-axis", "text-anchor": "middle" }, `F2${unit} ←`),
        svg("text", { x: 14, y: BOX.top + BOX.height / 2, class: "vw-axis", "text-anchor": "middle", transform: `rotate(-90 14 ${BOX.top + BOX.height / 2})` }, `F1${unit} →`),
      );
      // A clipped layer, so a wide ellipse never draws over the axes.
      const clip = svg("clipPath", { id: "vw-clip" });
      clip.append(svg("rect", { x: BOX.left, y: BOX.top, width: BOX.width, height: BOX.height }));
      const layer = svg("g", { "clip-path": "url(#vw-clip)" });
      root.append(svg("defs", {}), layer);
      root.querySelector("defs")!.append(clip);
      // The vowel space: the hull of each speaker's means, under everything else.
      for (const [list, two] of [[gs, false], [gs2, true]] as const) {
        const corners = hull(list.filter(spansTheSpace));
        if (corners.length < 3) continue;
        const pts = corners.map((g) => place(g.x, g.y, rg, BOX)).map(({ px, py }) => `${px.toFixed(1)},${py.toFixed(1)}`).join(" ");
        layer.append(svg("polygon", { points: pts, class: two ? "vw-hull vw-two" : "vw-hull" }));
      }
      for (const g of gs) {
        // Traced in data units and placed point by point, so the flipped,
        // unequally scaled axes draw it exactly.
        const d = ellipsePath(g)
          .map((q, i) => {
            const { px, py } = place(q.x, q.y, rg, BOX);
            return `${i === 0 ? "M" : "L"}${px.toFixed(1)},${py.toFixed(1)}`;
          })
          .join("");
        if (d !== "") layer.append(svg("path", { d: `${d}Z`, class: "vw-ellipse" }));
      }
      if (showTokens) {
        for (const p of ps) {
          const { px, py } = place(p.x, p.y, rg, BOX);
          const dot = svg("circle", { cx: px, cy: py, r: 3.2, class: "vw-dot", tabindex: 0 });
          dot.append(svg("title", {}, `${p.group}: F1 ${Math.round(p.point.f1!)} Hz, F2 ${Math.round(p.point.f2!)} Hz — click to hear it`));
          const open = () => void hear({ textId: r.textId, tierId: r.tierId, segmentId: p.point.segment, startMs: p.point.startMs, endMs: p.point.endMs });
          dot.addEventListener("click", open);
          dot.addEventListener("keydown", (ev) => {
            if ((ev as KeyboardEvent).key === "Enter") open();
          });
          if (getSelection()?.segmentId === p.point.segment) dot.classList.add("is-selected");
          layer.append(dot);
        }
      }
      if (showPaths) {
        // An arrowhead for each label's movement from 20% to 80% of its segments.
        const marker = svg("marker", { id: "vw-head", viewBox: "0 0 10 10", refX: 9, refY: 5, markerWidth: 7, markerHeight: 7, orient: "auto-start-reverse" });
        marker.append(svg("path", { d: "M0,0 L10,5 L0,10 z", class: "vw-head" }));
        root.querySelector("defs")!.append(marker);
        for (const t of trajectories(r.points, normalize)) {
          const a = place(t.from.x, t.from.y, rg, BOX);
          const b = place(t.to.x, t.to.y, rg, BOX);
          const line = svg("line", { x1: a.px, y1: a.py, x2: b.px, y2: b.py, class: "vw-path", "marker-end": "url(#vw-head)" });
          line.append(svg("title", {}, `${t.label}: from 20% to 80% of ${t.count} ${t.count === 1 ? "segment" : "segments"}`));
          layer.append(line, svg("circle", { cx: a.px, cy: a.py, r: 2.4, class: "vw-path-start" }));
        }
      }
      for (const g of gs) {
        const { px, py } = place(g.x, g.y, rg, BOX);
        layer.append(svg("text", { x: px, y: py + 5, class: "vw-label", "text-anchor": "middle" }, g.label.length > 12 ? `${g.label.slice(0, 11)}…` : g.label));
      }
      if (o !== null) {
        for (const g of gs2) {
          const d = ellipsePath(g)
            .map((q, i) => {
              const { px, py } = place(q.x, q.y, rg, BOX);
              return `${i === 0 ? "M" : "L"}${px.toFixed(1)},${py.toFixed(1)}`;
            })
            .join("");
          if (d !== "") layer.append(svg("path", { d: `${d}Z`, class: "vw-ellipse vw-two" }));
        }
        if (showTokens) {
          for (const p of ps2) {
            const { px, py } = place(p.x, p.y, rg, BOX);
            const dot = svg("circle", { cx: px, cy: py, r: 3.2, class: "vw-dot vw-two", tabindex: 0 });
            dot.append(svg("title", {}, `${o.track}, ${p.group}: F1 ${Math.round(p.point.f1!)} Hz, F2 ${Math.round(p.point.f2!)} Hz — click to hear it`));
            dot.addEventListener("click", () => void hear({ textId: o.textId, tierId: o.tierId, segmentId: p.point.segment, startMs: p.point.startMs, endMs: p.point.endMs }));
            layer.append(dot);
          }
        }
        for (const g of gs2) {
          const { px, py } = place(g.x, g.y, rg, BOX);
          layer.append(svg("text", { x: px, y: py + 5, class: "vw-label vw-two", "text-anchor": "middle" }, g.label.length > 12 ? `${g.label.slice(0, 11)}…` : g.label));
        }
        // Which is which, in the top corner.
        const key = svg("g", { class: "vw-key" });
        key.append(
          svg("circle", { cx: BOX.left + 12, cy: BOX.top + 12, r: 4, class: "vw-dot" }),
          svg("text", { x: BOX.left + 22, y: BOX.top + 16, class: "vw-key-text" }, r.track),
          svg("circle", { cx: BOX.left + 12, cy: BOX.top + 30, r: 4, class: "vw-dot vw-two" }),
          svg("text", { x: BOX.left + 22, y: BOX.top + 34, class: "vw-key-text" }, o.track),
        );
        root.append(key);
      }
      return root;
    };

    const paint = () => {
      const s = getState();
      if (s.kind !== "open") {
        el.replaceChildren(emptyState("No project open", "Open a project to plot its vowels."));
        return;
      }
      if (s.text === null) {
        el.replaceChildren(emptyState("Vowel space", "Select a text whose recording is segmented, one vowel to a segment, to plot F1 against F2."));
        return;
      }
      const timedTiers = s.text.tiers.filter((t) => s.text!.segments.some((x) => x.tier === t.id && x.startMs !== null && x.endMs !== null && x.endMs > x.startMs));
      if (tier === null || !timedTiers.some((t) => t.id === tier)) tier = trackToMeasure(s.text.tiers, s.text.segments, getSelection()?.segmentId ?? null)?.id ?? null;
      if (timedTiers.length === 0) {
        el.replaceChildren(emptyState("Vowel space", "This text has no timed segments yet. Segment the recording on the Timeline, one vowel (or one word) to a segment, then plot it here."));
        return;
      }
      const picker = h("select", { title: "The track to plot: each timed segment is one point, grouped by its label" }, ...timedTiers.map((t) => h("option", { value: t.id, textContent: t.name })));
      picker.value = tier ?? "";
      picker.onchange = () => {
        tier = picker.value;
        paint();
      };
      const otherPicker = h(
        "select",
        { className: "vw-other", title: "Another speaker's track to draw over this one, in its own colour and dashed; each is normalized against their own vowels" },
        h("option", { value: "", textContent: "No one" }),
        ...timedTiers.filter((t) => t.id !== tier).map((t) => h("option", { value: t.id, textContent: t.name, selected: t.id === other })),
      );
      otherPicker.setAttribute("aria-label", "Compare with");
      otherPicker.onchange = () => {
        other = otherPicker.value;
        if (other !== "") void run();
        else paint();
      };
      const go = h("button", { className: "primary", textContent: busy ? "Measuring…" : "Plot", disabled: busy, title: "Measure every timed segment of this track and plot F1 against F2" });
      go.onclick = () => void run();
      const norm = h("select", { className: "vw-method", title: METHODS.find((m) => m.id === normalize)?.about ?? "" }, ...METHODS.map((m) => h("option", { value: m.id, textContent: m.label, title: m.about, selected: m.id === normalize })));
      norm.setAttribute("aria-label", "Scale");
      norm.onchange = () => {
        normalize = parseMethod(norm.value);
        localStorage.setItem(NORMALIZE_KEY, normalize);
        paint();
      };
      const tok = h("input", { type: "checkbox", checked: showTokens });
      tok.onchange = () => {
        showTokens = tok.checked;
        localStorage.setItem(TOKENS_KEY, showTokens ? "on" : "off");
        paint();
      };
      const paths = h("input", { type: "checkbox", checked: showPaths });
      paths.onchange = () => {
        showPaths = paths.checked;
        localStorage.setItem(PATHS_KEY, showPaths ? "on" : "off");
        paint();
      };
      const current = result !== null && result.key === `${s.text.id}|${tier}` ? result : null;
      const copy = h("button", { textContent: "Copy table", disabled: current === null, title: "Copy every segment's label, times, F0 and F1–F3 as a table for a spreadsheet or R" });
      copy.onclick = () => {
        if (current === null) return;
        void writeText(table(current.points)).then(
          () => showToast({ tone: "info", lines: [`Copied ${current.points.length} rows.`] }),
          (e) => showToast({ tone: "error", lines: [errorText(e)] }),
        );
      };
      const save = h("button", { textContent: "Save picture…", disabled: current === null, title: "Save the plot as an SVG file for a paper or slides (opens in Word, Inkscape, Illustrator and browsers)" });
      save.onclick = () => {
        const shownPlot = el.querySelector<SVGSVGElement>(".vw-plot");
        if (current === null || shownPlot === null) return;
        const contents = standalone(shownPlot);
        void (async () => {
          const folder = await pickFile({ directory: true, title: "Where should the picture go?" });
          if (typeof folder !== "string") return;
          try {
            const r = await saveDocument({ folder, title: `${current.track} vowel space${normalize === "hz" ? "" : ` (${METHODS.find((m) => m.id === normalize)?.label ?? normalize})`}`, extension: "svg", contents });
            showToast({ tone: "info", lines: [`Saved the picture as ${r.path}.`] });
          } catch (e) {
            showToast({ tone: "error", lines: [errorText(e)] });
          }
        })();
      };
      const bar = h(
        "div",
        { className: "vw-bar" },
        h("label", {}, "Track ", picker),
        h("label", {}, "Compare with ", otherPicker),
        go,
        h("label", { title: "Hz as measured, Bark, or normalized so speakers can be compared" }, "Scale ", norm),
        h("label", { title: "Show a dot for every segment, not only each label's mean" }, tok, " Every token"),
        h("label", { title: "An arrow for each label from F1/F2 at 20% of its segments to F1/F2 at 80%: how a diphthong, or any vowel, moves" }, paths, " Trajectories"),
        copy,
        save,
      );
      const body: Node[] = [];
      if (problem !== null) body.push(h("p", { className: "note vw-problem", textContent: problem }));
      if (current !== null) {
        const shown = plotted(current.points, "hz").length;
        const notes = [plotNote(current.points, shown, current.untimed, current.analysedBy)];
        if (normalize === "watt-fabricius") {
          const s = wattCentroid(current.points.filter((p): p is Point & { f1: number; f2: number } => p.f1 !== null && p.f2 !== null && p.f1 > 0 && p.f2 > 0));
          notes.push(s === null ? "Watt–Fabricius needs at least two vowels to find the triangle." : `The triangle's corners: /${s.i}/ as /i/ and /${s.a}/ as /a/; its centre at F1 ${Math.round(s.f1)} Hz, F2 ${Math.round(s.f2)} Hz.`);
        }
        if (otherResult !== null && otherResult.key === `${current.textId}|${other}`) {
          const n2 = plotted(otherResult.points, "hz").length;
          notes.push(`Over it, ${otherResult.track}: ${n2} ${n2 === 1 ? "segment" : "segments"}, dashed.${normalize === "hz" || normalize === "bark" ? " In Hz or Bark, a longer vocal tract alone moves every vowel; choose a normalization to compare the vowels themselves." : " Each speaker is normalized against their own vowels."}`);
        }
        const areaOf = (ps: readonly Point[]) => vowelArea(groups(plotted(ps, normalize)), normalize);
        const second = otherResult !== null && otherResult.key === `${current.textId}|${other}` ? { track: otherResult.track, area: areaOf(otherResult.points) } : undefined;
        body.push(plot(), h("p", { className: "note", textContent: notes.join(" ") }), h("p", { className: "note vw-area", textContent: areaLine({ track: current.track, area: areaOf(current.points) }, second) }));
      } else if (problem === null) {
        body.push(
          h("p", {
            className: "note",
            textContent: "Plot measures every timed segment of the track (its middle half, the steady part of a vowel) and puts it at its F1 and F2. Label segments with the vowel they hold, and each label gets its mean and spread.",
          }),
        );
      }
      el.replaceChildren(bar, ...body);
    };

    const render = () => {
      const s = getState();
      const now = s.kind === "open" ? `${s.info.projectId}|${s.text?.id ?? ""}|${JSON.stringify(s.text?.segments.length ?? 0)}` : "closed";
      if (now !== lastText) {
        lastText = now;
        problem = null;
      }
      paint();
    };

    const unprovide = provide("vowels", () => [{ id: "vowels:plot", title: "Plot the vowel space", group: "Vowel space", words: ["formant", "f1", "f2", "vowel chart", "phonetics"], run: () => void run() }]);
    return { element: el, render, dispose: unprovide };
  },
};
