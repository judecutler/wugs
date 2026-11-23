// Orthography: write a transcription track in another orthography, such as
// IPA from a practical spelling, with an orthography profile (the table of
// letters the `segments` package and CLDF read). The profile is kept with
// the project; converted text goes on its own track, where corrections are
// kept when the track is converted again (src-tauri/src/orthography.rs,
// wugs_core::converted). Words in ../orthography/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, orthographyConvert, orthographyDraft, orthographyGet, orthographyPreview, orthographySave } from "../api";
import { emptyState, h } from "../dom";
import { checkLine, convertedLines, defaultColumn, defaultSource, defaultTarget, fromGrid, previewNote, toGrid, unknownLine, type Check, type Grid, type Preview, type Track } from "../orthography/logic";
import { provide } from "../palette/registry";
import { getState, refreshAll, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";

const CASE_KEY = "wugs.orthography.ignoreCase";

export const orthographyPanel: PanelDef = {
  title: "Orthography",
  create(): PanelView {
    const el = h("div", { className: "panel panel-orthography" });
    let project = "";
    let tracks: Track[] = [];
    let source: string | null = null;
    let onlyThisText = false;
    let ignoreCase = localStorage.getItem(CASE_KEY) !== "off";
    /** The profile being edited; `saved` is what the project holds. */
    let grid: Grid = toGrid("");
    let saved = "";
    let check: Check = { columns: [], rules: 0, problems: [] };
    let column: string | null = null;
    let target = "";
    let asText = false;
    let preview: Preview | null = null;
    let problem: string | null = null;
    let busy = false;
    let armedDraft = false;

    const profile = () => fromGrid(grid);
    const shownText = () => {
      const s = getState();
      return s.kind === "open" && s.text !== null ? s.text : null;
    };
    const sourceName = () => tracks.find((t) => t.id === source)?.name ?? "";
    const textId = () => (onlyThisText ? (shownText()?.id ?? null) : null);

    const load = async () => {
      try {
        const st = await orthographyGet();
        tracks = st.tracks;
        if (source === null || !tracks.some((t) => t.id === source)) source = defaultSource(tracks);
        saved = st.profile;
        grid = toGrid(st.profile);
        check = st.check;
        column = defaultColumn(check.columns);
        target = column ? defaultTarget(column, sourceName(), tracks.map((t) => t.name)) : "IPA";
        problem = null;
      } catch (e) {
        problem = errorText(e);
      }
      paint();
    };

    /** Columns as the grid has them, so a column typed into the header is offered at once. */
    const columns = () => grid.header.filter((c) => c.trim() !== "" && !/^(grapheme|frequency|comments?|notes?|left|right|class|codepoints|unicode|examples?)$/i.test(c.trim()));

    const run = async (what: () => Promise<void>) => {
      if (busy) return;
      busy = true;
      paint();
      try {
        await what();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      } finally {
        busy = false;
        paint();
      }
    };

    const doPreview = () =>
      run(async () => {
        if (source === null || column === null) return;
        preview = await orthographyPreview({ profile: profile(), tierId: source, textId: textId(), column, ignoreCase });
        check = preview.check;
      });

    const doSave = () =>
      run(async () => {
        check = await orthographySave({ profile: profile() });
        saved = profile();
        showToast({ tone: "info", lines: ["The orthography profile is kept with the project."] });
      });

    const doConvert = () =>
      run(async () => {
        if (source === null || column === null) return;
        const c = await orthographyConvert({ profile: profile(), tierId: source, textId: textId(), column, ignoreCase, target });
        saved = profile();
        showToast({ tone: c.unknown.length > 0 ? "warn" : "info", lines: convertedLines(c) });
        void refreshAll();
        const st = await orthographyGet();
        tracks = st.tracks;
        check = st.check;
      });

    const doDraft = () =>
      run(async () => {
        if (source === null) return;
        const text = await orthographyDraft({ tierId: source, textId: textId(), ignoreCase });
        grid = toGrid(text);
        column = "IPA";
        target = defaultTarget("IPA", sourceName(), tracks.map((t) => t.name));
        armedDraft = false;
        preview = null;
        showToast({ tone: "info", lines: [`${grid.rows.length} letters from «${sourceName()}», each written as itself. Change the IPA column where the spelling differs, add rows for letter groups such as «sh», then preview.`] });
      });

    const gridEditor = (): HTMLElement => {
      const head = h("tr", {}, ...grid.header.map((c) => h("th", { scope: "col", textContent: c })), h("th", { "aria-label": "Remove" } as Record<string, string>));
      const body = h("tbody", {});
      const row = (r: string[], i: number) => {
        const cells = r.map((c, j) => {
          const input = h("input", { type: "text", value: c, className: j === 0 ? "or-grapheme" : "" });
          input.setAttribute("aria-label", `${grid.header[j] ?? "Cell"}, row ${i + 1}`);
          input.spellcheck = false;
          input.oninput = () => {
            r[j] = input.value;
          };
          return h("td", {}, input);
        });
        const del = h("button", { type: "button", className: "or-del", textContent: "×", title: "Remove this row" });
        del.tabIndex = -1;
        del.onclick = () => {
          grid.rows.splice(grid.rows.indexOf(r), 1);
          paint();
        };
        return h("tr", {}, ...cells, h("td", {}, del));
      };
      body.append(...grid.rows.map(row));
      const add = h("button", { type: "button", textContent: "Add a row" });
      add.onclick = () => {
        grid.rows.push(grid.header.map(() => ""));
        paint();
        const inputs = el.querySelectorAll<HTMLInputElement>(".or-grapheme");
        inputs[inputs.length - 1]?.focus();
      };
      return h("div", {}, h("div", { className: "or-scroll" }, h("table", { className: "or-grid" }, h("thead", {}, head), body)), add);
    };

    const textEditor = (): HTMLElement => {
      const area = h("textarea", { className: "or-text", value: profile(), rows: 14 });
      area.setAttribute("aria-label", "The profile as text, a row a line, cells separated by tabs");
      area.spellcheck = false;
      area.onchange = () => {
        grid = toGrid(area.value);
      };
      return h("div", {}, area, h("p", { className: "note", textContent: "A row a line, cells separated by tabs: paste a profile from a spreadsheet or a .tsv file here." }));
    };

    const paint = () => {
      if (getState().kind !== "open") return;
      if (problem !== null) {
        const retry = h("button", { type: "button", textContent: "Look again" });
        retry.onclick = () => void load();
        el.replaceChildren(emptyState("Orthography", problem), retry);
        return;
      }
      if (tracks.length === 0) {
        el.replaceChildren(emptyState("No transcriptions yet", "Transcribe or import some texts, then convert their spelling here."));
        return;
      }
      const pick = h("select", {});
      pick.setAttribute("aria-label", "Track to convert");
      for (const t of tracks) {
        const o = h("option", { value: t.id, textContent: `${t.name} (${t.segments.toLocaleString()})` });
        o.selected = t.id === source;
        pick.append(o);
      }
      pick.onchange = () => {
        source = pick.value;
        if (column !== null) target = defaultTarget(column, sourceName(), tracks.map((t) => t.name));
        preview = null;
        paint();
      };
      const scope = h("select", {});
      scope.setAttribute("aria-label", "Texts");
      const shown = shownText();
      scope.append(h("option", { value: "all", textContent: "Every text" }), h("option", { value: "one", textContent: shown ? `Only «${shown.title}»` : "Only the text on screen" }));
      scope.value = onlyThisText ? "one" : "all";
      scope.onchange = () => {
        onlyThisText = scope.value === "one";
        preview = null;
      };
      const caseBox = h("input", { type: "checkbox", checked: ignoreCase });
      caseBox.onchange = () => {
        ignoreCase = caseBox.checked;
        localStorage.setItem(CASE_KEY, ignoreCase ? "on" : "off");
      };

      const draftBtn = h("button", { type: "button", textContent: armedDraft ? `Replace the profile with «${sourceName()}»'s letters` : "Start from this track's letters", disabled: busy });
      draftBtn.onclick = () => {
        if (grid.rows.length > 0 && !armedDraft) {
          armedDraft = true;
          paint();
          return;
        }
        void doDraft();
      };
      const modeBtn = h("button", { type: "button", textContent: asText ? "Edit as a table" : "Edit as text" });
      modeBtn.onclick = () => {
        asText = !asText;
        paint();
      };
      const copy = h("button", { type: "button", textContent: "Copy profile" });
      copy.title = "The profile as tab-separated text, for a .tsv file or the segments package";
      copy.onclick = () => writeText(profile()).then(() => showToast({ tone: "info", lines: ["The profile was copied as tab-separated text."] }), (e) => showToast({ tone: "error", lines: [errorText(e)] }));
      const save = h("button", { type: "button", textContent: profile() === saved ? "Kept with the project" : "Keep with the project", disabled: busy || profile() === saved });
      save.onclick = () => void doSave();

      const cols = columns();
      if (column === null || !cols.includes(column)) column = defaultColumn(cols);
      const colPick = h("select", {});
      colPick.setAttribute("aria-label", "Column to write");
      for (const c of cols) {
        const o = h("option", { value: c, textContent: c });
        o.selected = c === column;
        colPick.append(o);
      }
      colPick.onchange = () => {
        column = colPick.value;
        target = defaultTarget(column, sourceName(), tracks.map((t) => t.name));
        paint();
      };
      const targetInput = h("input", { type: "text", value: target, className: "or-target" });
      targetInput.setAttribute("aria-label", "Track to write");
      targetInput.oninput = () => {
        target = targetInput.value;
      };
      const ready = source !== null && column !== null && grid.rows.length > 0;
      const previewBtn = h("button", { type: "button", textContent: "Preview", disabled: busy || !ready });
      previewBtn.onclick = () => void doPreview();
      const convertBtn = h("button", { type: "button", className: "primary", textContent: busy ? "Working…" : `Write the «${target.trim() || "…"}» track`, disabled: busy || !ready || target.trim() === "" });
      convertBtn.onclick = () => void doConvert();

      const previewBox = preview
        ? h(
            "div",
            { className: "or-preview" },
            h("p", { className: "note", textContent: previewNote(preview) }),
            preview.unknown.length > 0 ? h("p", { className: "or-unknown", textContent: unknownLine(preview.unknown) }) : null,
            h("table", { className: "or-changes" }, h("thead", {}, h("tr", {}, h("th", { scope: "col", textContent: sourceName() }), h("th", { scope: "col", textContent: column ?? "" }))), h("tbody", {}, ...preview.changes.map((c) => h("tr", {}, h("td", { textContent: c.before }), h("td", { className: "or-after", textContent: c.after }))))),
          )
        : null;

      const parts: (HTMLElement | null)[] = [
        h("p", { className: "note", textContent: "Write a track in another spelling, such as IPA from a practical orthography, with an orthography profile: a table of letters and letter groups and what each is written as. The converted text goes on a track of its own; corrections made there are kept when you convert again." }),
        h("div", { className: "or-bar" }, h("label", {}, "Track ", pick), scope, h("label", { className: "or-check" }, caseBox, " Read capitals as small letters")),
        h("h3", { className: "or-title", textContent: "Profile" }),
        h("p", { className: "note", textContent: checkLine(check) + (grid.rows.length > 0 && profile() !== saved ? " Not kept with the project yet." : "") }),
        check.problems.length > 0 ? h("ul", { className: "or-problems" }, ...check.problems.map((p) => h("li", { textContent: p }))) : null,
        h("div", { className: "or-bar" }, draftBtn, modeBtn, copy, save),
        grid.rows.length === 0 && !asText ? h("p", { className: "note", textContent: "Start from the track's letters, add rows by hand, or paste a profile with Edit as text. Longer groups win: a row for «sh» is used before «s» and «h». Write ^ before a grapheme for the start of a word only and $ after it for the end only; NULL writes nothing, and an empty cell keeps the letter as it is." }) : null,
        asText ? textEditor() : gridEditor(),
        h("h3", { className: "or-title", textContent: "Convert" }),
        h("div", { className: "or-bar" }, h("label", {}, "Write the ", colPick, " column"), h("label", {}, "onto the track ", targetInput), previewBtn, convertBtn),
        previewBox,
      ];
      el.replaceChildren(...parts.filter((x): x is HTMLElement => x !== null));
    };

    const draw = () => {
      const s = getState();
      if (s.kind !== "open") {
        project = "";
        el.replaceChildren(emptyState("No project open", "Open a project to convert its spelling."));
        return;
      }
      if (s.info.projectId === project) return;
      project = s.info.projectId;
      source = null;
      preview = null;
      armedDraft = false;
      void load();
    };

    const unprovide = provide("orthography", () => [{ id: "orthography:preview", title: "Preview the orthography conversion", group: "Orthography", words: ["ipa", "spelling", "profile", "transliterate", "convert", "segments"], run: () => void doPreview() }]);
    return { element: el, render: draw, dispose: unprovide };
  },
};
