// Notes: every note in the project in one list (src-tauri/src/notes_overview.rs):
// the notes under sentences (Gloss panel) and on lexicon entries and senses,
// so the questions for the next session can be worked through, copied or
// printed. Read-only here; a sentence opens in the Gloss panel and an entry
// in the Lexicon, where the notes are edited. Rules in ../notes/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { allNotes, errorText, projectLanguage, saveDocument } from "../api";
import { localDay } from "../dates";
import { emptyState, h } from "../dom";
import { entryPlace, notesLine, notesSheet, notesText, shown, WHICH, type Filter, type Notes } from "../notes/logic";
import { provide } from "../palette/registry";
import { dataGeneration, getState, lexiconRevision, openEntry, selectSegment, selectText, showToast } from "../store";
import { SHOW_PANEL, type PanelDef, type PanelView } from "./panel";

export const notesPanel: PanelDef = {
  title: "Notes",
  create(): PanelView {
    const el = h("div", { className: "panel panel-notes" });
    let data: Notes | null = null;
    let seq = 0;
    let shownFor = "";
    const filter: Filter = { which: "all", query: "", questions: false };

    const chips = h("div", { className: "scopes", role: "radiogroup", ariaLabel: "Which notes" });
    const query = h("input", { type: "search", className: "nt-filter", placeholder: "Find in the notes", spellcheck: false });
    query.setAttribute("aria-label", "Find in the notes");
    const questionsBox = h("input", { type: "checkbox" });
    const questions = h("label", { className: "nt-questions", title: "Only notes that ask something (with a question mark)" }, questionsBox, " Only questions");
    const copy = h("button", { type: "button", textContent: "Copy list", title: "Copy the notes shown as plain text, grouped by text, to paste into a notebook or an email" });
    const print = h("button", { type: "button", textContent: "Print a list…", title: "Save the notes shown as a page to print, with a box to tick and room to write the answer beside each" });
    const again = h("button", { type: "button", textContent: "Read again", title: "Read the notes again after adding some in the Gloss panel or the Lexicon" });
    const line = h("p", { className: "note nt-line" });
    const list = h("div", { className: "nt-list" });

    const now = (): Notes => (data === null ? { sentences: [], entries: [] } : shown(data, filter));

    const drawChips = () =>
      chips.replaceChildren(
        ...WHICH.map(([id, label]) => {
          const b = h("button", { type: "button", textContent: label });
          b.setAttribute("role", "radio");
          b.setAttribute("aria-checked", String(id === filter.which));
          b.onclick = () => {
            filter.which = id;
            drawChips();
            draw();
          };
          return b;
        }),
      );

    const openSentence = async (textId: string, tierId: string, segmentId: string) => {
      await selectText(textId);
      selectSegment({ textId, tierId, segmentId });
      window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "gloss" }));
    };

    const draw = () => {
      if (data === null) {
        list.replaceChildren();
        return;
      }
      const n = now();
      line.textContent = notesLine(data, n);
      const parts: HTMLElement[] = [];
      let title: string | null = null;
      for (const s of n.sentences) {
        if (s.textTitle !== title) {
          title = s.textTitle;
          parts.push(h("h3", { className: "nt-head", textContent: s.textTitle || "Untitled text" }));
        }
        const where = h("button", { type: "button", className: "link-button nt-where", textContent: s.content || "(an empty sentence)", title: "Open this sentence in the Gloss panel" });
        where.onclick = () => void openSentence(s.textId, s.tierId, s.segmentId);
        parts.push(h("div", { className: "nt-item" }, where, h("p", { className: "nt-body", textContent: s.body })));
      }
      if (n.entries.length > 0) {
        parts.push(h("h3", { className: "nt-head", textContent: "Lexicon" }));
        for (const e of n.entries) {
          const where = h("button", { type: "button", className: "link-button nt-where", textContent: entryPlace(e), title: "Open this entry in the Lexicon" });
          where.onclick = () => {
            openEntry(e.entryId);
            window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "lexicon" }));
          };
          parts.push(h("div", { className: "nt-item" }, where, h("p", { className: "nt-body", textContent: e.body })));
        }
      }
      list.replaceChildren(...parts);
      copy.disabled = print.disabled = parts.length === 0;
    };

    async function load() {
      const mine = ++seq;
      try {
        const r = await allNotes();
        if (mine !== seq) return;
        data = r;
      } catch (e) {
        if (mine !== seq) return;
        data = null;
        line.textContent = `Could not read the notes: ${errorText(e)}`;
      }
      draw();
    }

    again.onclick = () => void load();
    query.oninput = () => {
      filter.query = query.value;
      draw();
    };
    questionsBox.onchange = () => {
      filter.questions = questionsBox.checked;
      draw();
    };
    copy.onclick = () => {
      void writeText(notesText(now())).then(
        () => showToast({ tone: "info", lines: ["Copied the notes shown."] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };
    print.onclick = async () => {
      try {
        const folder = await pickFolder({ directory: true, title: "Where should the list go?" });
        if (typeof folder !== "string") return;
        const language = await projectLanguage();
        const html = notesSheet(now(), { language: language.name, date: localDay(new Date()), generator: "wugs" });
        const r = await saveDocument({ folder, title: `${language.name.trim() || "Project"} notes`, extension: "html", contents: html });
        showToast({ tone: "info", lines: [`Saved the list in ${r.path}. Open it in a browser and print it for the session.`, ...r.notes] });
      } catch (e) {
        showToast({ tone: "error", lines: [`Could not save the list: ${errorText(e)}`] });
      }
    };

    const unprovide = provide("notes", () => [
      { id: "notes:questions", title: "Show only the questions in the notes", group: "Notes", words: ["consultant", "speaker", "ask", "session", "todo"], run: () => { questionsBox.checked = true; questionsBox.onchange?.(new Event("change")); } },
      { id: "notes:print", title: "Print the notes for a session", group: "Notes", words: ["questions", "consultant", "speaker", "sheet"], run: () => print.click() },
    ]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}|${lexiconRevision()}|${s.texts.map((t) => `${t.id}:${t.segments}`).join(",")}` : "closed";
      if (key === shownFor) return;
      const sameProject = s.kind === "open" && shownFor.split("|")[0] === s.info.projectId;
      shownFor = key;
      if (s.kind !== "open") {
        seq += 1;
        data = null;
        el.replaceChildren(emptyState("Notes", "Open a project to see its notes.", "The wug keeps a notebook too."));
        return;
      }
      if (!sameProject) {
        data = null;
        query.value = filter.query = "";
        questionsBox.checked = filter.questions = false;
        filter.which = "all";
        drawChips();
        el.replaceChildren(h("div", { className: "cc-bar" }, query, questions, again, copy, print), chips, line, list);
      }
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
