// Sessions: the fieldwork log (src-tauri/src/field_sessions.rs). One card per
// sitting: its date, place and method, who took part (from the People panel),
// notes (what was covered, what to ask next time), and the texts dated that
// day. Fields save on change or on leaving them; a redraw waits while one is
// being typed in. Words and tables in ../sessions/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { errorText, fieldSessions, projectLanguage, saveDocument } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { METHODS, dateLabel, perPerson, sessionsLine, sessionsSheet, sessionsTsv, shownSessions, today, type SessionView, type SessionsView } from "../sessions/logic";
import { dataGeneration, getState, selectText, showToast } from "../store";
import { SHOW_PANEL, type PanelDef, type PanelView } from "./panel";

export const sessionsPanel: PanelDef = {
  title: "Sessions",
  create(): PanelView {
    const el = h("div", { className: "panel panel-sessions" });
    let view: SessionsView | null = null;
    let shownFor = "";
    let seq = 0;
    let query = "";
    let person: string | null = null;
    const armed = new Set<string>();
    const projectPath = () => {
      const s = getState();
      return s.kind === "open" ? s.info.path : null;
    };

    const editing = () => {
      const a = document.activeElement;
      return (a instanceof HTMLInputElement || a instanceof HTMLTextAreaElement || a instanceof HTMLSelectElement) && el.contains(a);
    };
    let pending = false;
    el.addEventListener("focusout", () =>
      window.setTimeout(() => {
        if (pending && !editing()) {
          pending = false;
          draw();
        }
      }),
    );

    async function act(command: Parameters<typeof fieldSessions>[0], args: Record<string, unknown> = {}, focusNew = false) {
      const path = projectPath();
      const mine = ++seq;
      if (command !== "field_sessions_view") armed.clear();
      try {
        const next = await fieldSessions(command, args);
        if (projectPath() !== path || mine !== seq) return;
        const before = new Set(view?.sessions.map((x) => x.id) ?? []);
        view = next;
        if (editing() && !focusNew) pending = true;
        else draw();
        if (focusNew) {
          const made = next.sessions.find((x) => !before.has(x.id));
          if (made !== undefined) el.querySelector<HTMLElement>(`[data-session="${made.id}"] .ss-notes`)?.focus();
        }
      } catch (e) {
        if (projectPath() === path) showToast({ tone: "error", lines: [errorText(e)] });
      }
    }

    const field = (s: SessionView, name: "date" | "place" | "method", label: string): HTMLElement => {
      const input = h("input", { className: `ss-${name}`, value: s[name], placeholder: name === "method" ? "elicitation, recording…" : name === "place" ? "where" : "", ariaLabel: label }) as HTMLInputElement;
      if (name === "date") input.type = "date";
      if (name === "method") input.setAttribute("list", "ss-methods");
      const save = () => {
        if (input.value.trim() !== s[name]) void act("field_session_set", { session: s.id, field: name, value: input.value });
      };
      input.onchange = save;
      input.onkeydown = (e) => {
        if (e.key === "Enter") input.blur();
      };
      return h("label", { className: "ss-field" }, h("span", { className: "ss-label", textContent: label }), input);
    };

    const card = (s: SessionView): HTMLElement => {
      const notes = h("textarea", { className: "ss-notes", rows: 4, placeholder: "What was covered, what went well, what to ask next time…", ariaLabel: "Notes" }) as HTMLTextAreaElement;
      notes.value = s.notes;
      notes.onblur = () => {
        if (notes.value.trim() !== s.notes) void act("field_session_set", { session: s.id, field: "notes", value: notes.value });
      };
      const people = h(
        "div",
        { className: "ss-people" },
        ...s.people.map(([id, name]) => {
          const off = h("button", { type: "button", className: "ss-off", textContent: "×", title: `Take ${name || "this person"} off this session`, ariaLabel: `Take ${name} off` });
          off.onclick = () => void act("field_session_person", { session: s.id, person: id, add: false });
          return h("span", { className: "ss-chip" }, name || "(no name)", off);
        }),
      );
      const others = (view?.people ?? []).filter(([id]) => !s.people.some(([p]) => p === id));
      if (others.length > 0) {
        const add = h("select", { className: "ss-add", ariaLabel: "Add a person" }, h("option", { value: "", textContent: "+ Person…" }), ...others.map(([id, name]) => h("option", { value: id, textContent: name || "(no name)" })));
        add.onchange = () => {
          if (add.value !== "") void act("field_session_person", { session: s.id, person: add.value, add: true });
        };
        people.append(add);
      } else if ((view?.people.length ?? 0) === 0) {
        const open = h("button", { type: "button", className: "link-button", textContent: "Add people in the People panel" });
        open.onclick = () => window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "people" }));
        people.append(open);
      }
      const texts =
        s.texts.length === 0
          ? null
          : h(
              "p",
              { className: "ss-texts" },
              "Texts that day: ",
              ...s.texts.flatMap(([id, title], i) => {
                const b = h("button", { type: "button", className: "link-button", textContent: title || "(untitled)", title: "Open this text" });
                b.onclick = () => void selectText(id);
                return i === 0 ? [b] : [", ", b];
              }),
            );
      const delId = `del:${s.id}`;
      const del = h("button", { type: "button", className: armed.has(delId) ? "danger" : "", textContent: armed.has(delId) ? "Really delete?" : "Delete", title: "Delete this session from the log (two clicks)" });
      del.onclick = () => {
        if (armed.has(delId)) void act("field_session_delete", { session: s.id });
        else {
          armed.clear();
          armed.add(delId);
          draw();
        }
      };
      const article = h(
        "article",
        { className: "ss-card" },
        h("h3", { className: "ss-title", textContent: dateLabel(s.date) }),
        h("div", { className: "ss-row" }, field(s, "date", "Date"), field(s, "place", "Place"), field(s, "method", "Method")),
        people,
        notes,
        ...(texts === null ? [] : [texts]),
        h("div", { className: "ss-foot" }, del),
      );
      article.dataset.session = s.id;
      return article;
    };

    function draw() {
      if (view === null) return;
      const v = view;
      const search = h("input", { type: "search", className: "ss-search", placeholder: "Find in the log", value: query, ariaLabel: "Find in the log" }) as HTMLInputElement;
      search.oninput = () => {
        query = search.value;
        drawList();
      };
      const who = h("select", { className: "ss-who", title: "Only the sessions with this person" }, h("option", { value: "", textContent: "Everyone" }), ...v.people.map(([id, name]) => h("option", { value: id, textContent: name || "(no name)" })));
      who.value = person ?? "";
      who.onchange = () => {
        person = who.value === "" ? null : who.value;
        drawList();
      };
      const add = h("button", { type: "button", className: "primary", textContent: "New session", title: "Log a session dated today" });
      add.onclick = () => void act("field_session_add", { date: today() }, true);
      const copy = h("button", { type: "button", textContent: "Copy table", title: "The sessions shown as a table: date, place, method, people, texts and notes" });
      copy.onclick = () =>
        void writeText(sessionsTsv(shownSessions(v.sessions, query, person))).then(
          () => showToast({ tone: "info", lines: ["Copied the log. Paste it into a spreadsheet."] }),
          (e) => showToast({ tone: "error", lines: [errorText(e)] }),
        );
      const print = h("button", { type: "button", textContent: "Print the log…", title: "Save the sessions shown as one page to print or keep with the recordings" });
      print.onclick = async () => {
        try {
          const folder = await pickFolder({ directory: true, title: "Where should the log go?" });
          if (typeof folder !== "string") return;
          const language = await projectLanguage();
          const r = await saveDocument({ folder, title: `${language.name.trim() || "Project"} fieldwork log`, extension: "html", contents: sessionsSheet(shownSessions(v.sessions, query, person), { language: language.name, generator: "wugs" }) });
          showToast({ tone: "info", lines: [`Saved the log in ${r.path}.`, ...r.notes] });
        } catch (e) {
          showToast({ tone: "error", lines: [`Could not save the log: ${errorText(e)}`] });
        }
      };
      const line = h("p", { className: "note ss-line" });
      const list = h("div", { className: "ss-list" });
      const withWhom = perPerson(v.sessions);
      const people = withWhom.length === 0 ? null : h("p", { className: "note ss-whom", textContent: `Worked with: ${withWhom.map((p) => `${p.name || "(no name)"} ×${p.sessions}`).join(", ")}.` });
      function drawList() {
        const shown = shownSessions(v.sessions, query, person);
        line.textContent = sessionsLine(v.sessions, shown.length);
        list.replaceChildren(...shown.map(card));
      }
      const methods = h("datalist", { id: "ss-methods" }, ...METHODS.map((m) => h("option", { value: m })));
      el.replaceChildren(h("div", { className: "cc-bar" }, add, search, who, copy, print), line, ...(people === null ? [] : [people]), methods, list);
      drawList();
    }

    const unprovide = provide("sessions", () => [
      { id: "sessions:new", title: "Log a new field session", group: "Sessions", words: ["fieldwork", "diary", "notebook", "consultant", "speaker", "log"], run: () => void act("field_session_add", { date: today() }, true) },
    ]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}` : "closed";
      if (key === shownFor) return;
      const sameProject = s.kind === "open" && shownFor.split("|")[0] === s.info.projectId;
      shownFor = key;
      if (s.kind !== "open") {
        seq += 1;
        view = null;
        el.replaceChildren(emptyState("Sessions", "Open a project to keep its fieldwork log.", "The wug dates every page."));
        return;
      }
      if (!sameProject) {
        view = null;
        query = "";
        person = null;
        el.replaceChildren(h("p", { className: "note", textContent: "Reading the log…" }));
      }
      void act("field_sessions_view");
    };
    return { element: el, render, dispose: unprovide };
  },
};
