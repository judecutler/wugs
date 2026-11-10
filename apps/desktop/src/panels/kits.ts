// The Kits panel (roadmap item 26), the Kit Builder: which kit is in use,
// the installed kits, and an editor for the user's own. A kit is a
// kit.toml (crates/wugs-kit); the form edits it in place, and View code
// shows the same file to read or edit by hand. Tongues is built in and
// read-only; "Duplicate" makes an editable copy.
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { errorText, kitCode, kitDelete, kitDuplicate, kitInstall, kitSave, kitSaveCode, type KitEntry, type KitsView } from "../api";
import { emptyState, h } from "../dom";
import { describeLayout, kitNotes, withoutPanel, type Kit } from "../kits/logic";
import { currentArrangement, reloadKits, useKit } from "../kits/workspace";
import { getKits, setKits, showToast } from "../store";
import { sliced, type PanelDef, type PanelView } from "./panel";
import { isPanelId, isPlatformPanel, PANELS, toolPanels } from "./registry";

const title = (panel: string) => (isPanelId(panel) ? PANELS[panel].title : panel);

// What the person is in the middle of lives here, not in a panel instance:
// switching kits rebuilds the dock, and with it this panel.
type Draft = { text: string; saved: string; problems: string[] };
/** Code views by kit: the text as typed, as last saved, and why a save was
 * refused. Kept when the view is hidden or another kit is shown. */
const drafts = new Map<string, Draft>();
const codeOpen = new Set<string>();
let selected: string | null = null;
/** Actions run one after another; a field saved while another save is in
 * flight waits for it instead of being dropped. */
let queue: Promise<void> = Promise.resolve();

export const kitsPanel: PanelDef = {
  title: "Kits",
  create(): PanelView {
    const el = h("div", { className: "panel panel-kits" });
    let busy = false;
    /** The kit a "Duplicate" name field is open for, and its typed name. */
    let duplicating: { source: string; name: string } | null = null;
    let armed: string | null = null;
    const dirty = (id: string) => {
      const d = drafts.get(id);
      return d !== undefined && d.text !== d.saved;
    };

    const entries = () => getKits()?.kits ?? [];
    const current = (): KitEntry | null => entries().find((k) => k.kit.id === (selected ?? getKits()?.active)) ?? entries()[0] ?? null;

    /** Run `f` after any earlier action. `quiet` actions (a field saved on
     * Tab) do not redraw on their own: the panel catches up once nobody is
     * typing in it, so the field the person moved to keeps their typing. */
    function act(f: () => Promise<void>, quiet = false): Promise<void> {
      const run = async () => {
        // A field's change event fires while Tab is between fields, with
        // focus on neither; let it land so the redraw waits for the typing.
        if (quiet) await new Promise((r) => window.setTimeout(r));
        busy = true;
        if (!quiet) draw();
        let failed = false;
        try {
          await f();
        } catch (e) {
          failed = true;
          showToast({ tone: "error", lines: errorText(e).split("\n") });
        } finally {
          busy = false;
          // A refused field must show what is saved, not what was typed.
          if (quiet && !failed) view.render();
          else draw();
        }
      };
      // One failing action must not stop the ones queued after it.
      queue = queue.then(run).catch(() => {});
      return queue;
    }

    const latest = (id: string) => entries().find((k) => k.kit.id === id)?.kit ?? null;
    const sameStart = (a: Kit, b: Kit) => JSON.stringify([a.layout, a.active]) === JSON.stringify([b.layout, b.active]);

    /** The kit in use keeps its live arrangement when its start changes;
     * Reset layout shows the new one. Said once, where it happened. */
    function startChanged(id: string, before: Kit, after: Kit, fromWorkspace: boolean) {
      if (fromWorkspace || sameStart(before, after) || getKits()?.active !== id) return;
      showToast({ tone: "info", lines: ["Saved. Reset layout shows the workspace as this kit now starts."] });
    }

    /** Save a change to a user kit, applied to its newest saved state, so
     * edits queued one after another never undo each other. */
    function save(id: string, change: (k: Kit) => Kit, opts: { quiet?: boolean; fromWorkspace?: boolean } = {}) {
      return act(async () => {
        const before = latest(id);
        if (before === null) return;
        // Typed code would be written over this change by its next save.
        if (dirty(id)) {
          showToast({ tone: "warn", lines: ["Not saved: the code view has unsaved changes. Save or discard them first."] });
          return;
        }
        const kit = change(before);
        if (JSON.stringify(kit) === JSON.stringify(before)) return;
        await kitSave({ id, kit });
        // The code view's text is refreshed before the panel redraws from
        // the new kit, so the two never disagree.
        if (!dirty(id) && drafts.has(id)) await loadCode(id);
        await reloadKits();
        startChanged(id, before, kit, opts.fromWorkspace === true);
      }, opts.quiet);
    }

    async function loadCode(id: string) {
      const text = await kitCode({ id });
      drafts.set(id, { text, saved: text, problems: [] });
    }

    // ------------------------------------------------------------------ drawing

    /** Redraw, keeping focus and the caret in the field that had them. */
    function draw() {
      const active = document.activeElement;
      const key = active instanceof HTMLElement && el.contains(active) ? active.dataset.focus : undefined;
      const caret = active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement ? [active.selectionStart, active.selectionEnd] : null;
      drawAll();
      if (key === undefined) return;
      const again = el.querySelector<HTMLElement>(`[data-focus="${CSS.escape(key)}"]`);
      again?.focus();
      if (caret !== null && (again instanceof HTMLInputElement || again instanceof HTMLTextAreaElement)) again.setSelectionRange(caret[0] ?? null, caret[1] ?? null);
    }

    function drawAll() {
      const view = getKits();
      if (view === null) {
        el.replaceChildren(emptyState("Kits", "Loading the installed kits…"));
        return;
      }
      const entry = current();
      el.replaceChildren(
        h("p", { className: "kit-intro", textContent: "A kit decides which tools are offered and how the workspace starts. Your data stays the same whichever kit you use." }),
        h("ul", { className: "kit-list" }, ...view.kits.map((k) => kitRow(k, view))),
        h(
          "div",
          { className: "kit-actions" },
          h("button", { type: "button", textContent: "Install a kit file…", disabled: busy, onclick: () => void install() }),
        ),
        ...(view.broken.length === 0 ? [] : [brokenList(view)]),
        ...(entry === null ? [] : [editor(entry)]),
      );
    }

    function kitRow(k: KitEntry, view: KitsView) {
      const inUse = k.kit.id === view.active;
      const isSelected = k.kit.id === current()?.kit.id;
      return h(
        "li",
        {
          className: isSelected ? "active" : "",
          tabIndex: 0,
          onclick: (e: MouseEvent) => {
            if ((e.target as HTMLElement).closest("button, input") !== null) return;
            selected = k.kit.id;
            draw();
          },
          onkeydown: (e: KeyboardEvent) => {
            if (e.key === "Enter" && e.target === e.currentTarget) {
              selected = k.kit.id;
              draw();
            }
          },
        },
        h("span", { className: "kit-name", textContent: k.kit.name }),
        k.kit.subtitle === "" ? null : h("span", { className: "kit-sub", textContent: k.kit.subtitle }),
        h("span", { className: k.bundled ? "badge badge-muted" : "badge", textContent: k.bundled ? "built in" : "yours" }),
        inUse ? h("span", { className: "badge badge-use", textContent: "in use" }) : null,
        h(
          "span",
          { className: "kit-row-actions" },
          inUse ? null : h("button", { type: "button", textContent: "Use", disabled: busy, title: `Switch the workspace to ${k.kit.name}`, onclick: () => void act(() => useKit(k.kit.id)) }),
          h("button", {
            type: "button",
            textContent: "Duplicate",
            disabled: busy,
            onclick: () => {
              duplicating = { source: k.kit.id, name: `My ${k.kit.name}` };
              draw();
              el.querySelector<HTMLInputElement>(".kit-dup input")?.select();
            },
          }),
          k.bundled ? null : confirmButton(`delete:${k.kit.id}`, "Delete", () => void remove(k.kit.id)),
        ),
        duplicating?.source === k.kit.id ? duplicateForm(k) : null,
      );
    }

    function duplicateForm(k: KitEntry) {
      const input = h("input", { value: duplicating?.name ?? "", placeholder: "Name of the new kit", maxLength: 40 });
      input.oninput = () => {
        if (duplicating !== null) duplicating.name = input.value;
      };
      const go = () =>
        act(async () => {
          const made = await kitDuplicate({ source: k.kit.id, name: input.value });
          duplicating = null;
          selected = made.kit.id;
          await reloadKits();
        });
      input.onkeydown = (e) => {
        if (e.key === "Enter") void go();
        if (e.key === "Escape") {
          duplicating = null;
          draw();
        }
      };
      return h(
        "div",
        { className: "kit-dup" },
        input,
        h("button", { type: "button", textContent: "Make copy", disabled: busy, onclick: () => void go() }),
        h("button", { type: "button", textContent: "Cancel", onclick: () => ((duplicating = null), draw()) }),
      );
    }

    function brokenList(view: KitsView) {
      return h(
        "div",
        { className: "kit-broken" },
        h("strong", { textContent: "Files in the kits folder that could not be used" }),
        ...view.broken.map((b) => h("div", {}, h("code", { textContent: b.file }), ...b.problems.map((p) => h("div", { className: "kit-problem", textContent: p })))),
      );
    }

    function editor(entry: KitEntry) {
      const kit = entry.kit;
      const notes = kitNotes(kit, isPanelId, isPlatformPanel);
      const codeDirty = dirty(kit.id);
      // While the code view holds unsaved edits, the form would change the
      // file under them; one way of editing at a time.
      const ro = entry.bundled || codeDirty;
      return h(
        "section",
        { className: "kit-editor" },
        h("h3", { textContent: kit.name }),
        entry.bundled ? h("p", { className: "kit-note", textContent: "Built in, so it cannot be changed here. Duplicate it to make a kit of your own." }) : null,
        codeDirty
          ? h(
              "p",
              { className: "kit-note kit-warn" },
              "The code below has unsaved changes. Save or discard them to use the form again. ",
              h("button", { type: "button", textContent: "Discard code changes", onclick: () => void act(() => loadCode(kit.id)) }),
            )
          : null,
        ...notes.map((n) => h("p", { className: "kit-note kit-warn", textContent: n })),
        textField(kit, "name", "Name", ro, 40),
        textField(kit, "subtitle", "Subtitle", ro, 40),
        textField(kit, "version", "Version", ro, 40),
        textField(kit, "description", "About", ro, 500),
        h("h4", { textContent: "Tools" }),
        h("p", { className: "kit-note", textContent: "Conflicts, Devices and Kits are always offered." }),
        h("div", { className: "kit-tools" }, ...toolPanels().map((id) => toolBox(kit, id, ro))),
        ...kit.tools.filter((t) => !isPanelId(t)).map((t) => h("div", { className: "kit-note" }, `${t}: not in this version`, ro ? null : h("button", { type: "button", className: "lx-x", textContent: "×", title: `Take ${t} out of the kit`, onclick: () => void save(kit.id, (k) => ({ ...k, tools: k.tools.filter((x) => x !== t) })) }))),
        h("h4", { textContent: "How the workspace starts" }),
        h("ol", { className: "kit-layout" }, ...describeLayout(kit.layout, title).map((line) => h("li", { textContent: line }))),
        ro
          ? null
          : h(
              "div",
              { className: "kit-actions" },
              h("button", {
                type: "button",
                textContent: "Use the current arrangement",
                disabled: busy,
                title: "Arrange the panels by dragging them, then click this to make that arrangement how this kit starts",
                onclick: () => void useArrangement(kit),
              }),
            ),
        codeView(kit, entry.bundled),
      );
    }

    function textField(kit: Kit, key: "name" | "subtitle" | "version" | "description", label: string, ro: boolean, max: number) {
      const input = h("input", { value: kit[key], readOnly: ro, maxLength: max });
      input.dataset.focus = `field:${kit.id}:${key}`;
      input.onchange = () => {
        const value = input.value.trim();
        if (key === "name" && value === "") {
          input.value = kit.name;
          showToast({ tone: "warn", lines: ["A kit needs a name."] });
          return;
        }
        void save(kit.id, (k) => ({ ...k, [key]: value }), { quiet: true });
      };
      input.onkeydown = (e) => {
        if (e.key === "Enter") input.blur();
      };
      return h("div", { className: "field-row" }, h("label", { textContent: label }), input);
    }

    function toolBox(kit: Kit, id: string, ro: boolean) {
      const box = h("input", { type: "checkbox", checked: kit.tools.includes(id), disabled: ro || busy });
      box.onchange = () => {
        if (box.checked) {
          void save(kit.id, (k) => ({ ...k, tools: k.tools.includes(id) ? k.tools : [...k.tools, id] }));
        } else {
          void save(kit.id, (k) => ({ ...k, tools: k.tools.filter((t) => t !== id), layout: withoutPanel(k.layout, id), active: k.active.filter((a) => a !== id) }));
        }
      };
      return h("label", { className: "kit-tool" }, box, ` ${title(id)}`);
    }

    function useArrangement(kit: Kit) {
      const live = currentArrangement();
      if (live === null) return;
      let layout = live.layout;
      for (const p of layout) {
        if (!isPlatformPanel(p.panel) && !kit.tools.includes(p.panel)) layout = withoutPanel(layout, p.panel);
      }
      if (layout.length === 0) {
        showToast({ tone: "warn", lines: ["Open at least one of this kit's panels first."] });
        return;
      }
      const placed = new Set(layout.map((p) => p.panel));
      void save(kit.id, (k) => ({ ...k, layout, active: live.active.filter((a) => placed.has(a)) }), { fromWorkspace: true });
    }

    function codeView(kit: Kit, ro: boolean) {
      const shownOpen = codeOpen.has(kit.id) && drafts.has(kit.id);
      const toggle = h("button", {
        type: "button",
        className: "kit-code-toggle",
        textContent: shownOpen ? "Hide code" : "View code",
        disabled: busy,
        onclick: () =>
          void act(async () => {
            if (shownOpen) {
              codeOpen.delete(kit.id);
              return;
            }
            if (!drafts.has(kit.id)) await loadCode(kit.id);
            codeOpen.add(kit.id);
          }),
      });
      const draft = () => drafts.get(kit.id)!;
      const shown = drafts.get(kit.id);
      if (!shownOpen || shown === undefined) return h("div", { className: "kit-actions" }, toggle, dirty(kit.id) ? h("span", { className: "kit-note kit-warn", textContent: "unsaved code changes" }) : null);
      const area = h("textarea", { className: "kit-code", value: shown.text, readOnly: ro, spellcheck: false, rows: 18 });
      area.dataset.focus = `code:${kit.id}`;
      const saveButton = h("button", { type: "button", textContent: "Save code", disabled: busy || shown.text === shown.saved, onclick: () => void saveCode() });
      const wasDirty = dirty(kit.id);
      area.oninput = () => {
        draft().text = area.value;
        saveButton.disabled = busy || draft().text === draft().saved;
        // Locking or unlocking the form needs a redraw; typing does not.
        if (dirty(kit.id) !== wasDirty) draw();
      };
      const saveCode = () =>
        act(async () => {
          const before = latest(kit.id);
          const d = draft();
          try {
            await kitSaveCode({ id: kit.id, code: d.text });
          } catch (e) {
            d.problems = errorText(e).split("\n");
            return;
          }
          d.problems = [];
          d.saved = d.text;
          await reloadKits();
          const after = latest(kit.id);
          if (before !== null && after !== null) startChanged(kit.id, before, after, false);
        });
      area.onkeydown = (e) => {
        if (e.key === "s" && (e.ctrlKey || e.metaKey)) {
          e.preventDefault();
          void saveCode();
        }
      };
      return h(
        "div",
        { className: "kit-code-box" },
        h("div", { className: "kit-actions" }, toggle, ro ? null : saveButton),
        h("p", { className: "kit-note", textContent: ro ? "kit.toml, as shipped." : "kit.toml. Saving checks it first; nothing is written until it is a valid kit." }),
        area,
        ...shown.problems.map((p) => h("div", { className: "kit-problem", textContent: p })),
      );
    }

    // ------------------------------------------------------------------ actions

    async function install() {
      const path = await pickFile({ multiple: false, directory: false, title: "Install a kit", filters: [{ name: "Kit manifest", extensions: ["toml"] }] });
      if (typeof path !== "string") return;
      await act(async () => {
        const made = await kitInstall({ path });
        selected = made.kit.id;
        await reloadKits();
        showToast({ tone: "info", lines: [`Installed ${made.kit.name}. Use it from the list when you are ready.`] });
      });
    }

    function remove(id: string) {
      return act(async () => {
        // The kit in use hands the workspace back to the built-in kit first,
        // so its arrangement is not saved again after the delete.
        const fallback = entries().find((k) => k.bundled)?.kit.id;
        if (getKits()?.active === id && fallback !== undefined) await useKit(fallback);
        setKits(await kitDelete({ id }));
        if (selected === id) selected = null;
        drafts.delete(id);
        codeOpen.delete(id);
      });
    }

    function confirmButton(key: string, label: string, run: () => void) {
      return h("button", {
        type: "button",
        className: "danger",
        disabled: busy,
        textContent: armed === key ? `${label}? Click again` : label,
        onclick: () => {
          if (armed === key) {
            armed = null;
            run();
            return;
          }
          armed = key;
          draw();
          window.setTimeout(() => {
            if (armed === key) {
              armed = null;
              draw();
            }
          }, 4000);
        },
      });
    }

    const view = sliced(el, getKits, draw, { holdWhileEditing: true });
    return view;
  },
};
