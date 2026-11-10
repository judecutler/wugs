// The Conflicts panel (roadmap item 24): every field two devices wrote without
// seeing each other's value, described in words, with its versions side by
// side. "Keep this" writes the chosen value, or the typed one, naming every
// open version, so the conflict closes on every device (DATA-MODEL §5.3); the
// other values stay in the log.
import { conflictResolve, conflictsList, errorText, type ConflictView, type VersionView } from "../api";
import { fieldsLabel, needsNumbers, typedRaw, versionMeta } from "../conflicts/logic";
import { emptyState, h } from "../dom";
import { dataGeneration, getState, refreshConflicts, refreshText, selectSegment, selectText, showToast, touchLexicon } from "../store";
import type { PanelDef, PanelView } from "./panel";

const PAGE = 100;

export const conflictsPanel: PanelDef = {
  title: "Conflicts",
  create(): PanelView {
    const el = h("div", { className: "panel panel-conflicts" });
    let shownFor = "";
    let items: ConflictView[] = [];
    let busy = false;
    let seq = 0;
    /** Cards drawn; more on request, so thousands of conflicts stay quick. */
    let limit = PAGE;
    /** Our own resolve is refreshing the count: its list is already here. */
    let ownResolve = false;
    /** Typed values per conflict, kept across redraws. */
    const typed = new Map<string, string>();
    const key = (c: ConflictView) => `${c.target}|${c.field}`;

    async function load() {
      const mine = ++seq;
      const path = projectPath();
      try {
        const list = await conflictsList();
        if (mine !== seq || projectPath() !== path) return;
        items = list;
        draw();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
    }

    const projectPath = () => {
      const s = getState();
      return s.kind === "open" ? s.info.path : null;
    };

    function setBusy(on: boolean) {
      busy = on;
      for (const b of el.querySelectorAll<HTMLButtonElement>(".cf-card button")) b.disabled = on;
    }

    async function resolve(c: ConflictView, raw: string) {
      if (busy) return;
      const path = projectPath();
      setBusy(true);
      try {
        const r = await conflictResolve({ target: c.target, field: c.field, raw, heads: c.heads });
        // Another project was opened meanwhile: this answer is not about it.
        if (projectPath() !== path) return;
        seq += 1;
        items = r.conflicts;
        // A refused (stale) choice keeps what was typed: the person did nothing wrong.
        const refused = r.notes.some((n) => n.includes("changed again since it was shown"));
        if (!refused) typed.delete(key(c));
        if (r.notes.length > 0) showToast({ tone: "warn", lines: r.notes });
        // The settled value now shows wherever the field is shown; the
        // count refresh below must not refetch the list we already have.
        touchLexicon();
        await refreshText();
        ownResolve = true;
        try {
          await refreshConflicts();
        } finally {
          ownResolve = false;
        }
        shownFor = renderKey();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      } finally {
        busy = false;
        draw();
      }
    }

    function versionRow(c: ConflictView, v: VersionView, i: number, numbered: boolean) {
      return h(
        "li",
        { className: v.winner ? "cf-version shown" : "cf-version" },
        h(
          "div",
          { className: "cf-value" },
          numbered ? h("span", { className: "lx-code", textContent: `${i + 1}. ` }) : null,
          h("span", { textContent: v.label }),
        ),
        h(
          "div",
          { className: "cf-meta" },
          h("span", { className: "note", textContent: versionMeta(v, Date.now()) }),
          v.winner ? h("span", { className: "access", textContent: "shown now" }) : null,
          h("span", { className: "gl-spacer" }),
          h("button", { type: "button", textContent: "Keep this", disabled: busy, onclick: () => void resolve(c, v.raw) }),
        ),
      );
    }

    function card(c: ConflictView) {
      const numbered = needsNumbers(c.versions.map((v) => v.label));
      const open =
        c.textId === null
          ? null
          : h("button", {
              type: "button",
              className: "gl-alt",
              textContent: "Show",
              title: "Open the text at this segment",
              onclick: () =>
                void (async () => {
                  await selectText(c.textId!);
                  if (c.segmentId !== null) {
                    const s = getState();
                    const seg = s.kind === "open" ? s.text?.segments.find((x) => x.id === c.segmentId) : undefined;
                    if (seg !== undefined) selectSegment({ textId: c.textId!, tierId: seg.tier, segmentId: seg.id });
                  }
                })(),
            });
      let editor: HTMLElement | null = null;
      if (c.editable) {
        const input = h("input", { value: typed.get(key(c)) ?? "", placeholder: "Or type the right value", ariaLabel: `New value for ${c.title}` });
        input.dataset.key = key(c);
        input.oninput = () => typed.set(key(c), input.value);
        const save = () => {
          if (input.value.trim() === "") return;
          void resolve(c, typedRaw(input.value));
        };
        input.onkeydown = (ev) => {
          if (!ev.isComposing && ev.key === "Enter") {
            ev.preventDefault();
            save();
          }
        };
        editor = h("div", { className: "form inline cf-type" }, input, h("button", { type: "button", textContent: "Use this", disabled: busy, onclick: save }));
      }
      return h(
        "section",
        { className: "cf-card" },
        h("div", { className: "cf-head" }, h("strong", { textContent: c.title }), h("span", { className: "note", textContent: c.place }), h("span", { className: "gl-spacer" }), open),
        h("ul", { className: "cf-versions" }, ...c.versions.map((v, i) => versionRow(c, v, i, numbered))),
        editor,
      );
    }

    function draw() {
      const s = getState();
      if (s.kind !== "open") {
        el.replaceChildren(emptyState("Conflicts", "Open a project to see edits that need a decision."));
        return;
      }
      if (items.length === 0) {
        el.replaceChildren(emptyState("No conflicts", "Every edit merged cleanly.", "The wug is content."));
        return;
      }
      // Keep the typing position: a redraw rebuilds the inputs.
      const a = document.activeElement;
      const focusKey = a instanceof HTMLInputElement && el.contains(a) ? a.dataset.key : undefined;
      const caret = a instanceof HTMLInputElement ? a.selectionStart : null;
      const more = items.length - limit;
      el.replaceChildren(
        h(
          "p",
          { className: "note" },
          `${fieldsLabel(items.length)}: two devices changed them without seeing each other's change. Keep one version or type the right value; the other versions stay in the history.`,
        ),
        ...items.slice(0, limit).map(card),
        ...(more > 0
          ? [h("button", { type: "button", textContent: `Show ${Math.min(more, PAGE)} more (${more} not shown)`, onclick: () => { limit += PAGE; draw(); } })]
          : []),
      );
      if (busy) setBusy(true);
      if (focusKey !== undefined) {
        const again = el.querySelector<HTMLInputElement>(`input[data-key="${CSS.escape(focusKey)}"]`);
        again?.focus();
        if (again && caret !== null) again.setSelectionRange(caret, caret);
      }
    }

    function renderKey(): string {
      const s = getState();
      return s.kind === "open" ? `${s.info.path}|${s.info.conflicts}|${dataGeneration()}` : "closed";
    }

    return {
      element: el,
      render() {
        const s = getState();
        const next = renderKey();
        if (next === shownFor) return;
        if (ownResolve) {
          shownFor = next;
          return;
        }
        if (shownFor.split("|")[0] !== next.split("|")[0]) {
          typed.clear();
          limit = PAGE;
          seq += 1;
        }
        shownFor = next;
        if (s.kind !== "open") {
          items = [];
          draw();
          return;
        }
        void load();
      },
    };
  },
};
