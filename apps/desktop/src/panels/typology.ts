// Typology: what Grambank or WALS says about the project's language (or any
// language looked up by name or Glottocode), feature by feature, beside how
// its relatives answer (src-tauri/src/typology.rs). For the project's own
// language each feature also takes the project's answer and a note, so the
// list is a questionnaire to fill in; the answers copy out as CLDF values.
// Rules in ../typology/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { errorText, glottologFind, projectLanguage, saveDocument, setTypologyAnswer, typologyAnswers, typologyMap, typologyProfile } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { getState, showToast } from "../store";
import { answeredCount, cldfValues, counts, differs, othersLine, ownLine, profileNote, profileTable, shown, SOURCES, unlike, type Answer, type Ours, type Profile, type Show, type Source } from "../typology/logic";
import { colourOf, legend, mapNote, mapView, type FeatureMap } from "../typology/map";
import type { PanelDef, PanelView } from "./panel";
import { standalone } from "./vowels";

const SVG = "http://www.w3.org/2000/svg";
function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>, text?: string): SVGElementTagNameMap[K] {
  const el = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  if (text !== undefined) el.textContent = text;
  return el;
}

/** A feature's map: the land, a dot per compared language coloured by its answer, the language ringed, and a legend. */
function drawMap(m: FeatureMap, ownValue: string | null): HTMLElement {
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

const SOURCE_KEY = "wugs.typology.source";

const SHOWS: [Show, string][] = [
  ["all", "Every feature"],
  ["open", "Still open"],
  ["unlike", "Unlike its relatives"],
  ["differs", "Ours differs"],
];

export const typologyPanel: PanelDef = {
  title: "Typology",
  create(): PanelView {
    const el = h("div", { className: "panel panel-typology" });
    let profile: Profile | null = null;
    let problem: string | null = null;
    let show: Show = "all";
    let filter = "";
    /** The feature whose map is open, and the map once read. */
    let mapOf: string | null = null;
    let mapData: FeatureMap | null = null;
    let project = "";
    let seq = 0;
    let lookup = "";
    let source: Source = localStorage.getItem(SOURCE_KEY) === "wals" ? "wals" : "grambank";
    /** The Glottocode on screen, so a change of dataset reads the same language. */
    let shownCode: string | null = null;
    /** The project's language's Glottocode: only its profile takes the project's answers. */
    let projectCode = "";
    let ours = new Map<string, Ours>();

    const load = async (glottocode: string) => {
      const mine = ++seq;
      try {
        const p = await typologyProfile({ glottocode, source });
        const answers = glottocode === projectCode ? await typologyAnswers({ source }) : [];
        shownCode = glottocode;
        if (mine !== seq) return;
        profile = p;
        ours = new Map(answers.map((o) => [o.id, o]));
        problem = null;
      } catch (e) {
        if (mine !== seq) return;
        problem = errorText(e);
      }
      paint();
    };

    const loadProjectLanguage = async () => {
      const mine = ++seq;
      try {
        const lang = await projectLanguage();
        if (mine !== seq) return;
        projectCode = lang.glottocode;
        if (lang.glottocode === "") {
          profile = null;
          problem = "Set the project's language with its Glottocode in the Texts panel, or look a language up here.";
          paint();
          return;
        }
        await load(lang.glottocode);
      } catch (e) {
        if (mine === seq) {
          problem = errorText(e);
          paint();
        }
      }
    };

    const find = async () => {
      const q = lookup.trim();
      if (q === "") return;
      try {
        const hits = await glottologFind({ query: q });
        const first = hits[0];
        if (first === undefined) {
          showToast({ tone: "info", lines: [`Glottolog has no language matching «${q}».`] });
          return;
        }
        await load(first.glottocode);
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
    };

    const bar = (p: Profile): HTMLElement => {
      const input = h("input", { type: "search", placeholder: "Another language: name or Glottocode", value: lookup });
      input.setAttribute("aria-label", "Look up a language");
      input.oninput = () => {
        lookup = input.value;
      };
      input.onkeydown = (e) => {
        if (e.key === "Enter" && !e.isComposing) void find();
      };
      const mine = h("button", { type: "button", textContent: "The project's language" });
      mine.onclick = () => void loadProjectLanguage();
      const isMine = p.glottocode === projectCode;
      const copy = h("button", { type: "button", textContent: "Copy table" });
      copy.onclick = () =>
        writeText(profileTable(p, isMine ? ours : undefined)).then(
          () => showToast({ tone: "info", lines: [`${p.features.length} features of ${p.name} copied as a table.`] }),
          (e) => showToast({ tone: "error", lines: [errorText(e)] }),
        );
      const values = h("button", { type: "button", textContent: "Copy answers as CLDF" });
      values.title = "The project's answers as a CLDF values.csv, the shape Grambank and WALS publish theirs in";
      values.onclick = () => {
        const n = answeredCount(p, ours);
        if (n === 0) {
          showToast({ tone: "info", lines: ["The project has not answered any feature yet."] });
          return;
        }
        writeText(cldfValues(p, ours)).then(
          () => showToast({ tone: "info", lines: [`${n} answers copied as CLDF values (ID, Language_ID, Parameter_ID, Value, Code_ID, Comment).`] }),
          (e) => showToast({ tone: "error", lines: [errorText(e)] }),
        );
      };
      const which = SOURCES.map(([id, label]) => {
        const b = h("button", { type: "button", className: "ty-chip", textContent: label });
        b.setAttribute("aria-pressed", String(source === id));
        b.onclick = () => {
          if (source === id) return;
          source = id;
          localStorage.setItem(SOURCE_KEY, id);
          if (shownCode !== null) void load(shownCode);
        };
        return b;
      });
      const group = h("div", { className: "ty-sources" }, ...which);
      group.setAttribute("role", "group");
      group.setAttribute("aria-label", "Dataset");
      return h("div", { className: "ty-bar" }, group, input, mine, copy, isMine ? values : null);
    };

    const paint = () => {
      if (getState().kind !== "open") return;
      if (profile === null) {
        const retry = h("button", { type: "button", textContent: "Look again" });
        retry.onclick = () => void loadProjectLanguage();
        const input = h("input", { type: "search", placeholder: "A language's name or Glottocode", value: lookup });
        input.setAttribute("aria-label", "Look up a language");
        input.oninput = () => {
          lookup = input.value;
        };
        input.onkeydown = (e) => {
          if (e.key === "Enter" && !e.isComposing) void find();
        };
        el.replaceChildren(emptyState("Typology", problem ?? "Reading Grambank…"), h("div", { className: "ty-bar" }, input, retry));
        return;
      }
      const p = profile;
      const isMine = p.glottocode === projectCode;
      if (!isMine && show === "differs") show = "all";
      const chipEls = new Map<Show, HTMLButtonElement>();
      const chips = SHOWS.filter(([id]) => isMine || id !== "differs").map(([id]) => {
        const b = h("button", { type: "button", className: "ty-chip" });
        b.setAttribute("aria-pressed", String(show === id));
        b.onclick = () => {
          show = id;
          paint();
        };
        chipEls.set(id, b);
        return b;
      });
      const answeredLine = h("p", { className: "note" });
      /** Counts follow the answers without redrawing the list under the person. */
      const recount = () => {
        const n = counts(p, isMine ? ours : undefined);
        for (const [id, label] of SHOWS) {
          const b = chipEls.get(id);
          if (b) b.textContent = `${label} (${n[id]})`;
        }
        answeredLine.textContent = isMine ? `This project has answered ${answeredCount(p, ours)} of ${p.features.length}. Answers are kept with the project and sync to its members.` : "";
      };
      recount();
      const save = async (a: Answer, value: string, note: string, item: HTMLElement) => {
        try {
          const o = await setTypologyAnswer({ source, id: a.id, value, note });
          if (profile !== p) return;
          if (o.value === "" && o.note === "") ours.delete(a.id);
          else ours.set(a.id, o);
          item.classList.toggle("is-differs", differs(a, ours));
          recount();
        } catch (e) {
          showToast({ tone: "error", lines: [errorText(e)] });
        }
      };
      const ourRow = (a: Answer, item: HTMLElement): HTMLElement => {
        const o = ours.get(a.id);
        const select = h("select", {});
        select.setAttribute("aria-label", `This project's answer to ${a.id}`);
        select.dataset.key = `v:${a.id}`;
        const options: [string, string][] = [["", "Not answered"], ...a.codes.map((c): [string, string] => [c.value, c.label || c.value]), ["?", "Not known"]];
        for (const [value, label] of options) {
          const opt = h("option", { value, textContent: label });
          if ((o?.value ?? "") === value) opt.selected = true;
          select.append(opt);
        }
        const note = h("input", { type: "text", placeholder: "Note: an example, a source and page", value: o?.note ?? "" });
        note.setAttribute("aria-label", `Note on ${a.id}`);
        note.dataset.key = `n:${a.id}`;
        let saved = o?.note ?? "";
        const saveNote = () => {
          if (note.value === saved) return;
          saved = note.value;
          void save(a, select.value, note.value, item);
        };
        select.onchange = () => void save(a, select.value, note.value, item);
        note.onkeydown = (e) => {
          if (e.key === "Enter" && !e.isComposing) saveNote();
        };
        note.onblur = saveNote;
        return h("div", { className: "ty-ours" }, h("span", { className: "ty-ours-label", textContent: "This project" }), select, note);
      };
      const search = h("input", { type: "search", placeholder: "Find a feature", value: filter });
      search.setAttribute("aria-label", "Find a feature");
      search.oninput = () => {
        filter = search.value;
        drawList();
      };
      const list = h("ul", { className: "ty-list" });
      const drawList = () => {
        const rows = shown(p, show, filter, isMine ? ours : undefined);
        list.replaceChildren(
          ...rows.map((a) => {
            const total = a.othersCoded;
            const segments = a.others.map((s) => {
              // The language's own answer, or for an uncoded language the project's.
              const own = a.value ?? (isMine ? ours.get(a.id)?.value : undefined);
              const seg = h("span", { className: `ty-seg${own === s.value ? " is-own" : ""}`, title: `${s.label || s.value}: ${s.count}` });
              seg.setAttribute("style", `flex:${s.count}`);
              return seg;
            });
            const item = h(
              "li",
              { className: `ty-item${unlike(a) ? " is-unlike" : ""}${isMine && differs(a, ours) ? " is-differs" : ""}` },
              h("div", { className: "ty-head" }, h("span", { className: "ty-id", textContent: a.id }), h("span", { className: "ty-q", textContent: a.question })),
              h("div", { className: "ty-answer" }, h("strong", { className: a.value === null || a.value === "?" ? "ty-open" : "", textContent: ownLine(a) }), h("span", { className: "note", textContent: `${p.comparedWith}: ${othersLine(a)}` })),
              total > 0 ? h("div", { className: "ty-dist", title: othersLine(a) }, ...segments) : null,
            );
            if (total > 0) {
              const open = mapOf === a.id;
              const toggle = h("button", { type: "button", className: "link-button ty-map-toggle", textContent: open ? "Hide map" : "Map", title: `Where the ${p.comparedWith} answer this, on a map` });
              toggle.setAttribute("aria-expanded", String(open));
              toggle.onclick = async () => {
                if (mapOf === a.id) {
                  mapOf = null;
                  drawList();
                  return;
                }
                mapOf = a.id;
                mapData = null;
                drawList();
                try {
                  const m = await typologyMap({ glottocode: p.glottocode, source, feature: a.id });
                  if (mapOf !== a.id || profile !== p) return;
                  mapData = m;
                  drawList();
                } catch (e) {
                  showToast({ tone: "error", lines: [errorText(e)] });
                }
              };
              item.querySelector(".ty-answer")?.append(toggle);
              if (open) {
                const own = a.value ?? (isMine ? (ours.get(a.id)?.value || null) : null);
                item.append(mapData !== null && mapData.feature === a.id ? drawMap(mapData, own) : h("p", { className: "note", textContent: "Drawing the map…" }));
              }
            }
            if (isMine) item.append(ourRow(a, item));
            return item;
          }),
        );
        if (rows.length === 0) list.append(h("li", { className: "note", textContent: "No feature here fits." }));
      };
      drawList();
      el.replaceChildren(
        bar(p),
        h("h3", { className: "ty-title", textContent: `${p.name} (${p.glottocode})` }),
        h("p", { className: "note", textContent: profileNote(p, source) }),
        answeredLine,
        h("div", { className: "ty-bar" }, ...chips, search),
        list,
        h("p", { className: "note ty-credit", textContent: `From ${p.edition}. Sources and comments for each value: ${p.page}` }),
      );
    };

    const draw = () => {
      const s = getState();
      if (s.kind !== "open") {
        project = "";
        profile = null;
        el.replaceChildren(emptyState("No project open", "Open a project to see its language's grammar features."));
        return;
      }
      if (s.info.projectId === project) return;
      project = s.info.projectId;
      profile = null;
      shownCode = null;
      problem = null;
      paint();
      void loadProjectLanguage();
    };

    const unprovide = provide("typology", () => [{ id: "typology:mine", title: "Show typology features for the project's language", group: "Typology", words: ["grambank", "wals", "atlas", "typology", "features", "grammar", "questionnaire"], run: () => void loadProjectLanguage() }]);
    return { element: el, render: draw, dispose: unprovide };
  },
};
