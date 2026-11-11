// The People panel (roadmap 61): who is in the project and what each agreed
// to, quick to fill in and never required. Records inform the corrections
// export's consent column and sealed packages; nothing is blocked by them
// (DECISIONS 2026-11-11, "Consent records inform, never enforce").
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { errorText, peopleCommand, type Coverable, type PeopleCommand, type PeopleConsent as ConsentView, type PeoplePerson as PersonView, type PeopleView } from "../api";
import { localDay } from "../dates";
import { emptyState, h } from "../dom";
import { termLabel, ROLES, type Answer } from "../people/logic";
import { dataGeneration, getState, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";


export const peoplePanel: PanelDef = {
  title: "People",
  create(): PanelView {
    const el = h("div", { className: "panel panel-people" });
    let view: PeopleView | null = null;
    let shownFor = "";
    let seq = 0;
    /** Delete buttons armed by a first click: person or record ids. */
    const armed = new Set<string>();

    const projectPath = () => {
      const s = getState();
      return s.kind === "open" ? s.info.path : null;
    };

    /** Run a command that returns the whole view; a project switched meanwhile drops the answer. */
    async function act(command: PeopleCommand, args: Record<string, unknown> = {}) {
      const path = projectPath();
      const mine = ++seq;
      try {
        const next = await peopleCommand(command, args);
        if (projectPath() !== path || mine !== seq) return;
        view = next;
        draw();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
    }

    function twoClick(id: string, label: string, run: () => void): HTMLButtonElement {
      const on = armed.has(id);
      return h("button", {
        type: "button",
        className: on ? "danger" : "",
        textContent: on ? `Really ${label.toLowerCase()}?` : label,
        onclick: () => {
          if (on) {
            armed.delete(id);
            run();
          } else {
            armed.add(id);
            draw();
          }
        },
      });
    }

    function consentCard(person: PersonView, c: ConsentView, coverable: Coverable[]): HTMLElement {
      const covers = h(
        "select",
        { onchange: () => void act("consent_set_covers", { consent: c.id, covers: covers.value }) },
        ...coverable.map((o) =>
          h("option", { value: o.id, selected: o.id === c.covers, textContent: o.kind === "project" ? o.label : `${o.kind === "recording" ? "Recording" : "Text"}: ${o.label}` }),
        ),
      );
      if (!coverable.some((o) => o.id === c.covers)) covers.append(h("option", { value: c.covers, selected: true, textContent: "(something deleted)" }));
      const date = h("input", { type: "date", value: c.date, title: "When they agreed" });
      date.onchange = () => void act("consent_set_date", { consent: c.id, date: date.value });
      const terms = c.terms.map(([term, answer]) =>
        h(
          "div",
          { className: "pp-term" },
          h("span", { className: "pp-term-label", textContent: termLabel(term) }),
          ...([
            [true, "Yes"],
            [false, "No"],
            [null, "Not asked"],
          ] as [Answer, string][]).map(([a, label]) =>
            h("button", {
              type: "button",
              className: answer === a ? "active" : "",
              textContent: label,
              ariaPressed: String(answer === a),
              onclick: () => {
                if (answer !== a) void act("consent_set_term", { consent: c.id, term, answer: a });
              },
            }),
          ),
        ),
      );
      const evidence = c.hasEvidence
        ? h(
            "div",
            { className: "pp-row" },
            h("span", { textContent: `Signed form: ${c.evidenceName ?? "attached"}` }),
            h("button", { type: "button", textContent: "Remove", title: "Detach it from this record. The file stays in the project's history.", onclick: () => void act("consent_detach", { consent: c.id }) }),
          )
        : h(
            "div",
            { className: "pp-row" },
            h("button", {
              type: "button",
              textContent: "Attach the signed form…",
              title: "A scan, photo or PDF of the form. It is kept in the project.",
              onclick: async () => {
                const path = await pickFile({ title: `The signed form for ${person.name}` });
                if (typeof path === "string") void act("consent_attach", { consent: c.id, path });
              },
            }),
          );
      return h(
        "div",
        { className: "pp-consent" },
        h("div", { className: "pp-row" }, h("span", { className: "pp-label", textContent: "Covers" }), covers, date),
        ...terms,
        evidence,
        h("div", { className: "pp-row" }, twoClick(c.id, "Delete record", () => void act("consent_delete", { consent: c.id }))),
      );
    }

    function personCard(p: PersonView, coverable: Coverable[]): HTMLElement {
      const name = h("input", { value: p.name, className: "pp-name", ariaLabel: "Name" });
      const saveName = () => {
        const next = name.value.trim();
        if (next !== "" && next !== p.name) void act("person_set_name", { person: p.id, name: next });
      };
      name.onkeydown = (ev) => {
        if (ev.key === "Enter") saveName();
      };
      name.onblur = saveName;
      const role = h(
        "select",
        { onchange: () => void act("person_set_roles", { person: p.id, roles: role.value === "" ? [] : [role.value] }) },
        h("option", { value: "", textContent: "No role" }),
        ...ROLES.map((r) => h("option", { value: r, textContent: r, selected: p.roles[0] === r })),
      );
      if (p.roles[0] !== undefined && !ROLES.includes(p.roles[0])) role.append(h("option", { value: p.roles[0], textContent: p.roles[0], selected: true }));
      return h(
        "section",
        { className: "pp-person" },
        h("div", { className: "pp-row" }, name, role, twoClick(p.id, "Delete", () => void act("person_delete", { person: p.id }))),
        ...p.consents.map((c) => consentCard(p, c, coverable)),
        h("button", {
          type: "button",
          textContent: p.consents.length === 0 ? "Record what they agreed to" : "Add another record",
          onclick: () => void act("consent_add", { person: p.id, covers: coverable[0]?.id ?? "", date: localDay(new Date()) }),
        }),
      );
    }

    function draw() {
      if (projectPath() === null) {
        el.replaceChildren(emptyState("No project open", "Open or create a project from the top bar."));
        return;
      }
      if (view === null) {
        el.replaceChildren(h("p", { className: "empty", textContent: "Loading…" }));
        return;
      }
      const v = view;
      const name = h("input", { placeholder: "Name", className: "pp-name" });
      const role = h("select", {}, ...ROLES.map((r) => h("option", { value: r, textContent: r })));
      const add = h(
        "form",
        {
          className: "pp-add",
          onsubmit: (ev: SubmitEvent) => {
            ev.preventDefault();
            if (name.value.trim() !== "") void act("person_add", { name: name.value, role: role.value });
          },
        },
        name,
        role,
        h("button", { type: "submit", textContent: "Add person" }),
      );
      el.replaceChildren(
        h(
          "p",
          { className: "pp-note" },
          "Optional. Write down who speaks in your recordings and what each person agreed to. The corrections export and sealed archive packages report what these records say; nothing in wugs is blocked by them. Your ethics board's rules are the ones that count.",
        ),
        add,
        ...(v.people.length === 0 ? [h("p", { className: "empty", textContent: "No one recorded yet." })] : v.people.map((p) => personCard(p, v.coverable))),
      );
    }

    return {
      element: el,
      render() {
        const s = getState();
        const next = s.kind === "open" ? `${s.info.path}|${dataGeneration()}|${s.texts.length}` : "closed";
        if (next === shownFor) return;
        if (shownFor.split("|")[0] !== next.split("|")[0]) {
          view = null;
          armed.clear();
        }
        shownFor = next;
        draw();
        if (s.kind === "open") void act("people_view");
      },
    };
  },
};
