// Typology: what Grambank says about the project's language (or any language
// looked up by name or Glottocode), feature by feature, beside how its
// relatives answer (src-tauri/src/grambank.rs). For a language Grambank has
// not coded, the open questions are a checklist for fieldwork. Rules in
// ../typology/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, glottologFind, grambankProfile, projectLanguage } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { getState, showToast } from "../store";
import { counts, grambankPage, othersLine, ownLine, profileNote, profileTable, shown, unlike, type Profile, type Show } from "../typology/logic";
import type { PanelDef, PanelView } from "./panel";

const SHOWS: [Show, string][] = [
  ["all", "Every feature"],
  ["open", "Still open"],
  ["unlike", "Unlike its relatives"],
];

export const typologyPanel: PanelDef = {
  title: "Typology",
  create(): PanelView {
    const el = h("div", { className: "panel panel-typology" });
    let profile: Profile | null = null;
    let problem: string | null = null;
    let show: Show = "all";
    let filter = "";
    let project = "";
    let seq = 0;
    let lookup = "";

    const load = async (glottocode: string) => {
      const mine = ++seq;
      try {
        const p = await grambankProfile({ glottocode });
        if (mine !== seq) return;
        profile = p;
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
      const copy = h("button", { type: "button", textContent: "Copy table" });
      copy.onclick = () =>
        writeText(profileTable(p)).then(
          () => showToast({ tone: "info", lines: [`${p.features.length} features of ${p.name} copied as a table.`] }),
          (e) => showToast({ tone: "error", lines: [errorText(e)] }),
        );
      return h("div", { className: "ty-bar" }, input, mine, copy);
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
      const n = counts(p);
      const chips = SHOWS.map(([id, label]) => {
        const b = h("button", { type: "button", className: "ty-chip", textContent: `${label} (${n[id]})` });
        b.setAttribute("aria-pressed", String(show === id));
        b.onclick = () => {
          show = id;
          paint();
        };
        return b;
      });
      const search = h("input", { type: "search", placeholder: "Find a feature", value: filter });
      search.setAttribute("aria-label", "Find a feature");
      search.oninput = () => {
        filter = search.value;
        drawList();
      };
      const list = h("ul", { className: "ty-list" });
      const drawList = () => {
        const rows = shown(p, show, filter);
        list.replaceChildren(
          ...rows.map((a) => {
            const total = a.othersCoded;
            const segments = a.others.map((s) => {
              const seg = h("span", { className: `ty-seg${a.value === s.value ? " is-own" : ""}`, title: `${s.label || s.value}: ${s.count}` });
              seg.setAttribute("style", `flex:${s.count}`);
              return seg;
            });
            return h(
              "li",
              { className: `ty-item${unlike(a) ? " is-unlike" : ""}` },
              h("div", { className: "ty-head" }, h("span", { className: "ty-id", textContent: a.id }), h("span", { className: "ty-q", textContent: a.question })),
              h("div", { className: "ty-answer" }, h("strong", { className: a.value === null || a.value === "?" ? "ty-open" : "", textContent: ownLine(a) }), h("span", { className: "note", textContent: `${p.comparedWith}: ${othersLine(a)}` })),
              total > 0 ? h("div", { className: "ty-dist", title: othersLine(a) }, ...segments) : null,
            );
          }),
        );
        if (rows.length === 0) list.append(h("li", { className: "note", textContent: "No feature here fits." }));
      };
      drawList();
      el.replaceChildren(
        bar(p),
        h("h3", { className: "ty-title", textContent: `${p.name} (${p.glottocode})` }),
        h("p", { className: "note", textContent: profileNote(p) }),
        h("div", { className: "ty-bar" }, ...chips, search),
        list,
        h("p", { className: "note ty-credit", textContent: `From ${p.edition}: Skirgård et al. 2023, Science Advances. Sources and comments for each value: ${grambankPage(p.codedAs ?? p.glottocode)}` }),
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
      problem = null;
      paint();
      void loadProjectLanguage();
    };

    const unprovide = provide("typology", () => [{ id: "typology:mine", title: "Show Grambank's features for the project's language", group: "Typology", words: ["grambank", "typology", "features", "grammar", "wals", "questionnaire"], run: () => void loadProjectLanguage() }]);
    return { element: el, render: draw, dispose: unprovide };
  },
};
