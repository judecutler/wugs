// Facsimile (the Glyphs kit's page viewer): the scans or photos a text was
// read from, page by page, with a box on the page for each segment marked
// on it. Click a box to select its segment; select a segment and drag on
// the page to mark where it stands. Pages and areas are kept by
// src-tauri/src/facsimile.rs; rules in ../facsimile/logic.ts.
import { convertFileSrc } from "@tauri-apps/api/core";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { errorText, facsimileAddPage, facsimileMovePage, facsimileRemovePage, facsimileSetArea, facsimileView } from "../api";
import { emptyState, h } from "../dom";
import { areasOn, facsimileLine, nextToMark, pageOf, pointIn, rectFrom, zoomBy, type Area, type FacsimileView, type Point } from "../facsimile/logic";
import { provide } from "../palette/registry";
import { dataGeneration, getSelection, getState, selectSegment, showToast, subscribeSelection } from "../store";
import type { PanelDef, PanelView } from "./panel";

const NEXT_KEY = "wugs.facsimile.next";

export const facsimilePanel: PanelDef = {
  title: "Facsimile",
  create(): PanelView {
    const el = h("div", { className: "panel panel-facsimile" });
    let view: FacsimileView | null = null;
    let textId: string | null = null;
    let shown = "";
    let seq = 0;
    let page = 0;
    let zoom = 1;
    let busy = false;
    let armedRemove: string | null = null;
    let goNext = localStorage.getItem(NEXT_KEY) !== "off";
    const pageFor = new Map<string, number>();

    const text = () => {
      const s = getState();
      return s.kind === "open" ? s.text : null;
    };

    const load = async () => {
      const t = text();
      const mine = ++seq;
      if (t === null) {
        view = null;
        paint();
        return;
      }
      try {
        const v = await facsimileView({ text: t.id });
        if (mine !== seq) return;
        view = v;
      } catch (e) {
        if (mine !== seq) return;
        view = null;
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      paint();
    };

    /** Run an edit that answers with the whole view. */
    const run = async (f: () => Promise<FacsimileView>) => {
      if (busy) return;
      busy = true;
      try {
        view = await f();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      } finally {
        busy = false;
      }
      paint();
    };

    const addPages = async () => {
      const t = text();
      if (t === null) return;
      const picked = await pickFile({ title: "Choose the page images, in order", multiple: true, directory: false, filters: [{ name: "Pictures", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }] });
      const paths = typeof picked === "string" ? [picked] : (picked ?? []);
      for (const path of paths) await run(() => facsimileAddPage({ text: t.id, path }));
      if (view !== null && paths.length > 0) page = view.pages.length - 1;
      paint();
    };

    const mark = (area: Area) => {
      const t = text();
      const sel = getSelection();
      if (t === null || sel === null || sel.textId !== t.id) return;
      void run(() => facsimileSetArea({ text: t.id, segment: sel.segmentId, area })).then(() => {
        if (!goNext || view === null) return;
        const next = nextToMark(t.segments.map((s) => ({ id: s.id, tier: s.tier })), view, sel.segmentId);
        if (next !== null) selectSegment({ textId: t.id, tierId: t.segments.find((s) => s.id === next)!.tier, segmentId: next });
      });
    };

    /** The page with its boxes, and a drag on it drawing the selected segment's area. */
    const stage = (v: FacsimileView, at: number) => {
      const p = v.pages[at]!;
      const t = text()!;
      const sel = getSelection();
      const wrap = h("div", { className: "fx-page" });
      wrap.style.width = `${zoom * 100}%`;
      if (p.path === null) {
        wrap.append(h("p", { className: "note", textContent: `${p.label} is not on this computer. It comes with the project's files when another device syncs them.` }));
        return wrap;
      }
      const img = h("img", { src: convertFileSrc(p.path), alt: p.label, draggable: false });
      wrap.append(img);
      for (const m of areasOn(v, p.source)) {
        const box = h("button", { type: "button", className: m.segment === sel?.segmentId ? "fx-area fx-on" : "fx-area", title: m.content || "(empty)" });
        box.style.left = `${m.area.x * 100}%`;
        box.style.top = `${m.area.y * 100}%`;
        box.style.width = `${m.area.w * 100}%`;
        box.style.height = `${m.area.h * 100}%`;
        box.setAttribute("aria-label", `Select «${m.content}»`);
        box.onclick = (ev) => {
          ev.stopPropagation();
          selectSegment({ textId: t.id, tierId: m.tier, segmentId: m.segment });
        };
        box.onpointerdown = (ev) => ev.stopPropagation();
        wrap.append(box);
      }
      const draft = h("div", { className: "fx-draft" });
      draft.hidden = true;
      wrap.append(draft);
      let from: Point | null = null;
      const at2 = (ev: PointerEvent) => pointIn(ev.clientX, ev.clientY, img.getBoundingClientRect());
      wrap.onpointerdown = (ev) => {
        const s = getSelection();
        if (ev.button !== 0 || s === null || s.textId !== t.id) return;
        from = at2(ev);
        wrap.setPointerCapture(ev.pointerId);
        ev.preventDefault();
      };
      wrap.onpointermove = (ev) => {
        if (from === null) return;
        const r = rectFrom(from, at2(ev), p.source);
        draft.hidden = r === null;
        if (r !== null) {
          draft.style.left = `${r.x * 100}%`;
          draft.style.top = `${r.y * 100}%`;
          draft.style.width = `${r.w * 100}%`;
          draft.style.height = `${r.h * 100}%`;
        }
      };
      wrap.onpointerup = (ev) => {
        if (from === null) return;
        const r = rectFrom(from, at2(ev), p.source);
        from = null;
        draft.hidden = true;
        if (r !== null) mark(r);
      };
      return wrap;
    };

    const paint = () => {
      const t = text();
      if (t === null) {
        el.replaceChildren(emptyState("Facsimile", "Open a text to see its page images and mark where each line stands on them.", "The wug is dusting off the tablet."));
        return;
      }
      if (view === null) {
        el.replaceChildren(emptyState("Facsimile", "Reading the pages…"));
        return;
      }
      const v = view;
      page = Math.min(Math.max(0, page), Math.max(0, v.pages.length - 1));
      const add = h("button", { type: "button", textContent: "Add pages…", title: "Add scans or photos of the text's pages (PNG, JPEG, GIF or WebP), in order, after the last page", disabled: busy });
      add.onclick = () => void addPages();
      const bar = h("div", { className: "cc-bar fx-bar" }, add);
      if (v.pages.length > 0) {
        const tabs = h(
          "div",
          { className: "fx-tabs", role: "tablist" },
          ...v.pages.map((p, i) => {
            const b = h("button", { type: "button", role: "tab", className: i === page ? "chip on" : "chip", textContent: String(i + 1), title: p.label });
            b.setAttribute("aria-selected", String(i === page));
            b.onclick = () => {
              page = i;
              pageFor.set(t.id, i);
              paint();
            };
            return b;
          }),
        );
        const cur = v.pages[page]!;
        const earlier = h("button", { type: "button", textContent: "◀", title: "Move this page one place earlier", disabled: busy || page === 0 });
        earlier.onclick = () => {
          page -= 1;
          void run(() => facsimileMovePage({ text: t.id, source: cur.source, by: -1 }));
        };
        const later = h("button", { type: "button", textContent: "▶", title: "Move this page one place later", disabled: busy || page === v.pages.length - 1 });
        later.onclick = () => {
          page += 1;
          void run(() => facsimileMovePage({ text: t.id, source: cur.source, by: 1 }));
        };
        const remove = h("button", { type: "button", className: armedRemove === cur.source ? "danger" : "", textContent: armedRemove === cur.source ? "Take it off?" : "Take this page off", title: "Take the page off this text; the areas marked on it are cleared. The image stays in the project", disabled: busy });
        remove.onclick = () => {
          if (armedRemove !== cur.source) {
            armedRemove = cur.source;
            paint();
            return;
          }
          armedRemove = null;
          void run(() => facsimileRemovePage({ text: t.id, source: cur.source }));
        };
        const out = h("button", { type: "button", textContent: "−", title: "Zoom out" });
        out.onclick = () => {
          zoom = zoomBy(zoom, -1);
          paint();
        };
        const fit = h("button", { type: "button", textContent: "Fit", title: "The page as wide as the panel" });
        fit.onclick = () => {
          zoom = 1;
          paint();
        };
        const inn = h("button", { type: "button", textContent: "+", title: "Zoom in" });
        inn.onclick = () => {
          zoom = zoomBy(zoom, 1);
          paint();
        };
        bar.append(tabs, earlier, later, remove, h("span", { className: "fx-zoom" }, out, fit, inn));
      }
      const sel = getSelection();
      const selected = sel !== null && sel.textId === t.id ? t.segments.find((s) => s.id === sel.segmentId) : undefined;
      const marked = selected === undefined ? undefined : v.marked.find((m) => m.segment === selected.id);
      const next = h("label", { className: "fx-next", title: "After marking a line, select the next line on the same track that has no area yet" }, h("input", { type: "checkbox", checked: goNext, onchange: (ev: Event) => {
        goNext = (ev.target as HTMLInputElement).checked;
        localStorage.setItem(NEXT_KEY, goNext ? "on" : "off");
      } }), " then the next line");
      const clear = marked === undefined ? null : h("button", { type: "button", textContent: "Clear its area", disabled: busy });
      if (clear !== null) clear.onclick = () => void run(() => facsimileSetArea({ text: t.id, segment: selected!.id, area: null }));
      const help =
        v.pages.length === 0
          ? null
          : h(
              "p",
              { className: "note fx-help" },
              selected === undefined ? "Select a line in Transcribe or Interlinear, then drag on the page to mark where it stands." : `${marked === undefined ? "Drag on the page to mark where" : "Drag again to move"} «${selected.content || "(empty)"}» ${marked === undefined ? "stands" : ""}`.trim() + ". ",
              ...(selected === undefined ? [] : [next]),
              ...(clear === null ? [] : [" ", clear]),
            );
      const pages = h("div", { className: "fx-scroll" }, ...(v.pages.length === 0 ? [] : [stage(v, page)]));
      el.replaceChildren(bar, h("p", { className: "note", textContent: facsimileLine(v, t.segments.length) }), ...(help === null ? [] : [help]), pages);
    };

    const follow = () => {
      // A segment selected elsewhere: show the page it is marked on.
      const sel = getSelection();
      if (view !== null && sel !== null) {
        const at = pageOf(view, sel.segmentId);
        if (at >= 0) page = at;
      }
      paint();
      el.querySelector(".fx-on")?.scrollIntoView({ block: "nearest", inline: "nearest" });
    };
    const unsubscribe = subscribeSelection(follow);
    const unprovide = provide("facsimile", () => [{ id: "facsimile:add", title: "Add page images to this text", group: "Facsimile", words: ["scan", "photo", "manuscript", "inscription", "image", "page"], run: () => void addPages() }]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${s.text?.id ?? ""}|${dataGeneration()}` : "closed";
      if (key === shown) return;
      const id = s.kind === "open" ? (s.text?.id ?? null) : null;
      if (id !== textId) {
        if (textId !== null) pageFor.set(textId, page);
        textId = id;
        page = id === null ? 0 : (pageFor.get(id) ?? 0);
        armedRemove = null;
        view = null;
      }
      shown = key;
      void load();
    };
    return {
      element: el,
      render,
      dispose: () => {
        unsubscribe();
        unprovide();
      },
    };
  },
};
