// A map of how languages answer one question (a Grambank or WALS feature,
// or whether a phoneme is in PHOIBLE's inventory): Natural Earth's land, a dot
// per language coloured by its answer, the language itself ringed, a legend
// and Save map…. Shared by the Typology and Sounds panels; placing and words
// in ../typology/map.ts.
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { errorText, saveDocument } from "../api";
import { h } from "../dom";
import { showToast } from "../store";
import { colourOf, legend, mapNote, mapView, type FeatureMap } from "../typology/map";
import { standalone } from "./vowels";

const SVG = "http://www.w3.org/2000/svg";
function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>, text?: string): SVGElementTagNameMap[K] {
  const el = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  if (text !== undefined) el.textContent = text;
  return el;
}

/** A feature's map: the land, a dot per compared language coloured by its answer, the language ringed, and a legend. */
export function drawFeatureMap(m: FeatureMap, ownValue: string | null): HTMLElement {
  const points = m.points.map((p) => (p.me ? { ...p, value: ownValue } : p));
  const view = mapView(points);
  const { x, y, w, h: hh } = view.box;
  const r = Math.max(w, hh) / 90;
  const root = svg("svg", { viewBox: `${x} ${y} ${w} ${hh}`, class: "tm-map", role: "img", "aria-label": `${m.question}: ${m.comparedWith} on a map` });
  root.append(svg("rect", { x, y, width: w, height: hh, class: "tm-sea" }));
  // The land twice, the second copy east of 180°, so a Pacific view has land on both sides.
  for (const dx of [0, 360]) root.append(svg("path", { d: m.land, transform: `translate(${dx} 0)`, class: "tm-land", "stroke-width": r / 4 }));
  for (const p of view.placed) {
    const dot = svg("circle", { cx: p.x, cy: p.y, r: p.me ? r * 1.4 : r, fill: colourOf(p.value, m.codes), class: p.me ? "tm-dot tm-me" : "tm-dot", "stroke-width": p.me ? r * 0.7 : r / 4 });
    const label = p.value === null ? "no answer" : p.value === "?" ? "not known" : m.codes.find((c) => c.value === p.value)?.label || p.value;
    dot.append(svg("title", {}, `${p.name}: ${label}`));
    root.append(dot);
  }
  const keys = h(
    "ul",
    { className: "tm-legend" },
    ...legend({ ...m, points }).map((row) =>
      {
        const swatch = h("span", { className: "tm-swatch" });
        swatch.style.background = row.colour;
        return h("li", { className: row.mine ? "is-mine" : "" }, swatch, `${row.label} (${row.count})${row.mine ? " ← this language" : ""}`);
      },
    ),
  );
  const save = h("button", { type: "button", className: "link-button", textContent: "Save map…", title: "Save the map as an SVG file for a paper or slides" });
  save.onclick = async () => {
    const folder = await pickFolder({ directory: true, title: "Where should the map go?" });
    if (typeof folder !== "string") return;
    try {
      const out = await saveDocument({ folder, title: `${m.feature} map`, extension: "svg", contents: standalone(root) });
      showToast({ tone: "info", lines: [`Saved the map as ${out.path}.`] });
    } catch (e) {
      showToast({ tone: "error", lines: [errorText(e)] });
    }
  };
  return h("div", { className: "tm-wrap" }, root, keys, h("p", { className: "note" }, mapNote({ ...m, points }), " ", save));
}
