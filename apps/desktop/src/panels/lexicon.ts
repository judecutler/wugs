// The Lexicon panel (roadmap item 22): browse and edit entries, their senses,
// glosses in a second language, definitions, and semantic domains. Glossing
// grows the lexicon (the Gloss panel's add-to-lexicon); this is where it is
// tidied. Every edit returns the entry's fresh view, so the editor redraws
// from one answer; warnings (a duplicate lemma) arrive as a toast. The
// entry's paradigm (roadmap item 50) is its own view, fetched for the chosen
// template and returned whole by every form edit.
import {
  domainsList, domainsLoadSil, errorText, lexiconAdd, lexiconAddSense, lexiconAssignDomain, lexiconDelete, lexiconDeleteSense, lexiconEntry,
  lexiconDeleteForm, lexiconFillCell, lexiconList, lexiconNumberHomographs, lexiconParadigm, lexiconSetField, lexiconSetHomograph, lexiconSetSenseGloss,
  lexiconSetSenseText, lexiconUnassignDomain,
  type DomainItem, type EntryEdit, type EntryRow, type EntryView, type LexemeField, type ParadigmEdit, type ParadigmForm, type ParadigmView, type SenseView,
} from "../api";
import { emptyState, h } from "../dom";
import { normalizeLangTag } from "../gloss/lang";
import { filterDomains, glossLine, headword, nextHomograph, parseHomograph, sameLemmaNote, usageLabel } from "../lexicon/logic";
import { axesOf, cellFeatures, cellKey, cellLabel, coverageLine, layout, otherFormLine, TEMPLATES, templateById, type Features, type Template } from "../lexicon/paradigms";
import { dataGeneration, getState, lexiconRevision, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";

/** Shared with the Gloss panel: the second analysis language. */
const LANG_KEY = "wugs.gloss-lang";
/** The paradigm template last chosen on this machine. */
const PARADIGM_KEY = "wugs.lexicon.paradigm";
const ATTRIBUTION = "SIL Semantic Domains v4 · © SIL Global · semdom.org · licensed CC BY-SA 4.0 (creativecommons.org/licenses/by-sa/4.0)";

export const lexiconPanel: PanelDef = {
  title: "Lexicon",
  create(): PanelView {
    const el = h("div", { className: "panel panel-lexicon" });
    const search = h("input", { type: "search", placeholder: "Find an entry by lemma or gloss" });
    const newLemma = h("input", { placeholder: "Lemma", ariaLabel: "New entry lemma" });
    const newGloss = h("input", { placeholder: "Gloss", ariaLabel: "New entry gloss" });
    const addForm = h(
      "form",
      { className: "inline lx-new" },
      newLemma,
      newGloss,
      h("button", { type: "submit", textContent: "Add entry" }),
    );
    const list = h("ul", { className: "lx-list" });
    const editor = h("div", { className: "lx-entry" });
    const body = h("div", { className: "lx-body" }, list, editor);

    let rows: EntryRow[] = [];
    let selected: string | null = null;
    let entry: EntryView | null = null;
    let domains: DomainItem[] | null = null;
    let template: Template = templateById(localStorage.getItem(PARADIGM_KEY));
    /** The selected entry's paradigm for `template`; null until it loads. */
    let paradigm: ParadigmView | null = null;
    /** The sense whose domain picker is open, and what was typed in it. */
    let picker: { senseId: string; query: string; active: number } | null = null;
    /** An armed destructive button: its key; a second click carries it out. */
    let armed: string | null = null;
    let listSeq = 0;
    /** True while the panel replaces its own nodes: the blur a removed input
     * fires then is not the user leaving the field, so it must not save. */
    let drawing = false;
    /** A background refresh that waited for the user to leave a field. */
    let refreshWaiting = false;
    /** Bumped on every project switch: an edit queued for the old project
     * must not adopt its answer into the new one's view. */
    let projectGen = 0;
    let queue: Promise<unknown> = Promise.resolve();
    let shownFor = "";

    // ------------------------------------------------------------------ data

    async function loadList() {
      const mine = ++listSeq;
      try {
        const r = await lexiconList({ query: search.value });
        if (mine !== listSeq) return;
        rows = r;
        drawList();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
    }

    async function loadEntry(id: string | null, keepUi = false) {
      if (!keepUi || id !== selected) {
        picker = null;
        armed = null;
      }
      selected = id;
      if (id === null) {
        entry = null;
        paradigm = null;
        quietly(drawAll);
        return;
      }
      const t = template;
      try {
        const [e, pd] = await Promise.all([lexiconEntry({ id }), lexiconParadigm({ id, axes: axesOf(t) }).catch(() => null)]);
        if (selected !== id) return;
        entry = e;
        if (template === t) paradigm = pd;
      } catch {
        // deleted on another device, or gone after a sync
        entry = null;
        paradigm = null;
        selected = null;
      }
      keepFocus(drawAll);
    }

    /** Fetch the selected entry's paradigm for the current template. */
    async function loadParadigm() {
      const id = selected;
      const t = template;
      if (id === null) return;
      try {
        const pd = await lexiconParadigm({ id, axes: axesOf(t) });
        if (selected !== id || template !== t) return;
        paradigm = pd;
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      keepFocus(drawEditor);
    }

    /** Replace the panel's own nodes without the removed fields' blurs saving. */
    function quietly(draw: () => void) {
      drawing = true;
      try {
        draw();
      } finally {
        drawing = false;
      }
    }

    async function ensureDomains(force = false): Promise<DomainItem[]> {
      const have = domains !== null && !force ? domains : await domainsList();
      domains = have;
      return have;
    }

    /** Edits run one after another, so the entry that comes back last is the
     * newest. A paradigm edit names the template it was made under: its
     * answer is adopted only while that template is still shown. */
    function run(job: () => Promise<EntryEdit | ParadigmEdit>, refreshList = false, under?: Template): Promise<void> {
      const gen = projectGen;
      const next = queue.then(async () => {
        if (gen !== projectGen) return;
        try {
          const r = await job();
          if (gen !== projectGen) return;
          if (r.notes.length > 0) showToast({ tone: "warn", lines: r.notes });
          if ("paradigm" in r) {
            if (r.paradigm.lexeme === selected) {
              if (under === template) paradigm = r.paradigm;
              else void loadParadigm();
            }
          } else if (r.entry === null) {
            entry = null;
            paradigm = null;
            selected = null;
          } else if (selected === r.entry.id || selected === null) {
            if (selected !== r.entry.id) {
              paradigm = null;
              selected = r.entry.id;
              void loadParadigm();
            }
            entry = r.entry;
          }
          if (refreshList) await loadList();
        } catch (e) {
          showToast({ tone: "error", lines: [errorText(e)] });
        }
        // A save started by a blur (Tab) can answer before focus has landed on
        // the next field; wait for that, so the redraw puts focus back there.
        await new Promise((r) => window.setTimeout(r, 0));
        keepFocus(drawAll);
      });
      queue = next;
      return next;
    }

    /** Redraw without throwing away where the user was typing: the focused
     * field comes back focused, with its unsaved text and its selection. */
    function keepFocus(draw: () => void) {
      const active = document.activeElement;
      const key = active instanceof HTMLElement && el.contains(active) ? active.dataset.key : undefined;
      const field = active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement ? active : null;
      const draft = field !== null && field.dataset.saved !== undefined && field.value !== field.dataset.saved ? field.value : null;
      const sel = field === null ? null : [field.selectionStart, field.selectionEnd] as const;
      drawing = true;
      try {
        draw();
      } finally {
        drawing = false;
      }
      if (key === undefined) return;
      const again = el.querySelector<HTMLElement>(`[data-key="${CSS.escape(key)}"]`);
      if (again === null) return;
      again.focus();
      if (again instanceof HTMLInputElement || again instanceof HTMLTextAreaElement) {
        if (draft !== null) again.value = draft;
        if (sel !== null && sel[0] !== null && sel[1] !== null) again.setSelectionRange(sel[0], sel[1]);
      }
    }

    /** A field in the editor has focus: a background redraw would pull the user out of it. */
    function editing(): boolean {
      const a = document.activeElement;
      return (a instanceof HTMLInputElement || a instanceof HTMLTextAreaElement) && editor.contains(a);
    }

    // ------------------------------------------------------------------ editing cells

    /** A one-line field that saves on Enter or when it loses focus, if changed. Escape restores. */
    function cell(key: string, value: string, label: string, save: (v: string) => Promise<void>, opts: { placeholder?: string; size?: number } = {}) {
      const input = h("input", { value, placeholder: opts.placeholder ?? "", ariaLabel: label });
      input.dataset.key = key;
      input.dataset.saved = value;
      if (opts.size !== undefined) input.size = opts.size;
      const commit = () => {
        if (drawing || input.value === input.dataset.saved) return;
        input.dataset.saved = input.value;
        void save(input.value);
      };
      input.onkeydown = (ev) => {
        if (ev.isComposing) return;
        if (ev.key === "Enter") {
          ev.preventDefault();
          commit();
        } else if (ev.key === "Escape") {
          input.value = input.dataset.saved ?? "";
          input.blur();
        }
      };
      input.onblur = commit;
      return input;
    }

    /** A multi-line field (definition, usage): saves when it loses focus, or on Ctrl/⌘+Enter. */
    function area(key: string, value: string, label: string, save: (v: string) => Promise<void>) {
      const ta = h("textarea", { value, placeholder: label, ariaLabel: label, rows: 2 });
      ta.dataset.key = key;
      ta.dataset.saved = value;
      const commit = () => {
        if (drawing || ta.value === ta.dataset.saved) return;
        ta.dataset.saved = ta.value;
        void save(ta.value);
      };
      ta.onkeydown = (ev) => {
        if (ev.key === "Enter" && (ev.ctrlKey || ev.metaKey)) {
          ev.preventDefault();
          commit();
        }
      };
      ta.onblur = commit;
      return ta;
    }

    function keyed<T extends HTMLElement>(key: string, node: T): T {
      node.dataset.key = key;
      return node;
    }

    function row(label: string, ...children: (Node | string)[]) {
      return h("div", { className: "field-row" }, h("label", { textContent: label }), ...children);
    }

    /** A button that needs a second click within a few seconds. */
    function confirmButton(key: string, label: string, act: () => void, opts: { armedText?: string; title?: string } = {}) {
      const isArmed = armed === key;
      return h("button", {
        type: "button",
        className: "danger",
        title: opts.title ?? "",
        textContent: isArmed ? (opts.armedText ?? `${label}? Click again`) : label,
        onclick: () => {
          if (armed === key) {
            armed = null;
            act();
            return;
          }
          armed = key;
          drawEditor();
          window.setTimeout(() => {
            if (armed === key) {
              armed = null;
              drawEditor();
            }
          }, 4000);
        },
      });
    }

    // ------------------------------------------------------------------ drawing

    function drawList() {
      if (rows.length === 0) {
        list.replaceChildren(
          h("li", { className: "empty", textContent: search.value.trim() === "" ? "No entries yet. Add one above, or gloss a word and add it from there." : "No entries match." }),
        );
        return;
      }
      list.replaceChildren(
        ...rows.map((r) =>
          h(
            "li",
            {
              className: r.id === selected ? "active" : "",
              tabIndex: 0,
              onclick: () => void loadEntry(r.id),
              onkeydown: (ev: KeyboardEvent) => {
                if (ev.key === "Enter") void loadEntry(r.id);
              },
            },
            h("span", { className: "lx-hw", textContent: headword(r.lemma, r.homograph) }),
            r.posOfSpeech === "" ? null : h("span", { className: "lx-pos", textContent: r.posOfSpeech }),
            h("span", { className: "lx-gl", textContent: glossLine(r.glosses) }),
          ),
        ),
      );
    }

    function drawEditor() {
      if (entry === null) {
        editor.replaceChildren(h("p", { className: "note", textContent: rows.length === 0 ? "" : "Pick an entry to edit it." }));
        return;
      }
      const e = entry;
      const lang = normalizeLangTag(localStorage.getItem(LANG_KEY) ?? "");
      const setField = (field: LexemeField) => (v: string) => run(() => lexiconSetField({ id: e.id, field, value: v }), true);
      const homograph = cell(`hom:${e.id}`, e.homograph === null ? "" : String(e.homograph), "Homograph number", async (v) => {
        const parsed = parseHomograph(v);
        if (!parsed.ok) {
          showToast({ tone: "error", lines: ["A homograph number is a whole number from 1, or empty."] });
          keepFocus(drawEditor);
          return;
        }
        await run(() => lexiconSetHomograph({ id: e.id, number: parsed.value }), true);
      }, { size: 3, placeholder: "–" });
      const twins = e.sameLemma.map(([, n]) => n);
      const note = sameLemmaNote(e.lemma, e.homograph, twins);
      const head = h(
        "section",
        { className: "lx-head" },
        h("h1", { textContent: headword(e.lemma, e.homograph) }),
        row("Lemma", cell(`lemma:${e.id}`, e.lemma, "Lemma", setField("lemma"))),
        row("Citation", cell(`cit:${e.id}`, e.citation, "Citation form", setField("citation"), { placeholder: "if it differs from the lemma" })),
        row("Category", cell(`pos:${e.id}`, e.posOfSpeech, "Part of speech", setField("pos-of-speech"), { placeholder: "n, v, adj…" })),
        row("Homograph", homograph),
        note === null
          ? null
          : h(
              "div",
              { className: "gl-notice" },
              h("span", { textContent: note }),
              e.homograph === null || twins.some((n) => n === null)
                ? h("button", {
                    type: "button",
                    textContent: "Number them",
                    title: `Give every «${e.lemma}» without a number the next free one (this one would be ${e.homograph ?? nextHomograph(twins)})`,
                    onclick: () => void run(() => lexiconNumberHomographs({ id: e.id }), true),
                  })
                : null,
            ),
      );
      const senses = e.senses.map((s, i) => senseBlock(e, s, i, lang));
      const foot = h(
        "div",
        { className: "lx-foot" },
        h("button", {
          type: "button",
          textContent: "Add a sense",
          onclick: () => void run(() => lexiconAddSense({ id: e.id, gloss: "" })),
        }),
        h("span", { className: "gl-spacer" }),
        confirmButton(`del:${e.id}`, "Delete entry", () => void run(() => lexiconDelete({ id: e.id }), true)),
      );
      editor.replaceChildren(head, ...senses, paradigmBlock(e), foot);
    }

    function paradigmBlock(e: EntryView) {
      const t = template;
      const select = h("select", { ariaLabel: "Paradigm template" }, ...TEMPLATES.map((x) => h("option", { value: x.id, textContent: x.label, selected: x.id === t.id })));
      select.dataset.key = `pdg-template:${e.id}`;
      select.onchange = () => {
        template = templateById(select.value);
        localStorage.setItem(PARADIGM_KEY, template.id);
        paradigm = null;
        keepFocus(drawEditor);
        void loadParadigm();
      };
      const pd = paradigm !== null && paradigm.lexeme === e.id ? paradigm : null;
      const head = h(
        "div",
        { className: "lx-pdg-head" },
        h("h2", { textContent: "Paradigm" }),
        select,
        h("span", { className: "note", textContent: pd === null ? "Loading…" : coverageLine(t, pd.cells) }),
      );
      if (pd === null) return h("section", { className: "lx-paradigm" }, head);
      const grid = layout(t, pd.cells);
      const cols = t.cols === null ? [null] : t.cols.values;
      const table = h(
        "table",
        { className: "lx-pdg" },
        t.cols === null ? null : h("thead", {}, h("tr", {}, h("th", {}), ...t.cols.values.map((v) => h("th", { scope: "col", textContent: v.label })))),
        h(
          "tbody",
          {},
          ...t.rows.values.map((r, ri) =>
            h(
              "tr",
              {},
              h("th", { scope: "row", textContent: r.label }),
              ...cols.map((c, ci) => h("td", {}, ...cellInputs(e, t, cellFeatures(t, r.value, c?.value ?? null), grid[ri]![ci]?.forms ?? []))),
            ),
          ),
        ),
      );
      const others =
        pd.others.length === 0
          ? null
          : h(
              "div",
              { className: "lx-pdg-others" },
              h("p", { className: "note", textContent: "Other inflected forms of this entry, not in this grid:" }),
              h(
                "ul",
                {},
                ...pd.others.map((f) =>
                  h("li", {}, otherFormLine(f), " ", confirmButton(`delf:${f.id}`, "Delete", () => void run(() => lexiconDeleteForm({ id: e.id, formId: f.id, axes: axesOf(t) }), false, t))),
                ),
              ),
            );
      return h("section", { className: "lx-paradigm" }, head, table, others);
    }

    /** A cell's fields: one per form it holds, or one empty field that adds
     * a form with the cell's features. Delete buttons are out of the Tab
     * order, so Tab walks from cell to cell. */
    function cellInputs(e: EntryView, t: Template, features: Features, forms: ParadigmForm[]): Node[] {
      const label = cellLabel(t, features);
      const axes = axesOf(t);
      if (forms.length === 0) {
        return [cell(cellKey(e.id, features, 0), "", label, (v) => run(() => lexiconFillCell({ id: e.id, formId: null, features, surface: v, axes }), false, t), { placeholder: "—", size: 12 })];
      }
      return forms.map((f, i) => {
        const del = confirmButton(`delf:${f.id}`, "×", () => void run(() => lexiconDeleteForm({ id: e.id, formId: f.id, axes }), false, t), {
          armedText: "Delete?",
          title: `Delete «${f.form}»`,
        });
        del.tabIndex = -1;
        return h(
          "span",
          { className: "lx-pdg-form" },
          cell(cellKey(e.id, features, i), f.form, label, (v) => run(() => lexiconFillCell({ id: e.id, formId: f.id, features, surface: v, axes }), false, t), { size: 12 }),
          del,
        );
      });
    }

    function senseBlock(e: EntryView, s: SenseView, i: number, lang: string | null) {
      const glossRow = row(
        e.senses.length > 1 ? `Sense ${i + 1}` : "Gloss",
        cell(`gl:${s.id}`, s.gloss, "Gloss", (v) => run(() => lexiconSetSenseGloss({ senseId: s.id, gloss: v, lang: null }), true)),
      );
      const second =
        lang === null
          ? null
          : row(
              lang,
              cell(`gl:${s.id}:${lang}`, s.glossesByLang[lang] ?? "", `Gloss (${lang})`, (v) =>
                run(() => lexiconSetSenseGloss({ senseId: s.id, gloss: v, lang })),
              ),
            );
      const others = Object.entries(s.glossesByLang).filter(([tag]) => tag !== lang);
      const chips = s.domains.map((d) =>
        h(
          "span",
          { className: "lx-chip", title: d.label },
          h("span", { className: "lx-code", textContent: d.code }),
          ` ${d.label} `,
          h("button", {
            type: "button",
            className: "lx-x",
            ariaLabel: `Remove ${d.label}`,
            textContent: "×",
            onclick: () => void run(() => lexiconUnassignDomain({ senseId: s.id, domainId: d.id })),
          }),
        ),
      );
      const open = picker?.senseId === s.id;
      return h(
        "section",
        { className: "lx-sense" },
        glossRow,
        second,
        others.length === 0 ? null : h("p", { className: "note", textContent: others.map(([t, g]) => `${t}: ${g}`).join(" · ") }),
        h("div", { className: "lx-def" }, area(`def:${s.id}`, s.definition, "Definition", (v) => run(() => lexiconSetSenseText({ senseId: s.id, field: "definition", text: v })))),
        h("div", { className: "lx-def" }, area(`use:${s.id}`, s.usage, "Usage note", (v) => run(() => lexiconSetSenseText({ senseId: s.id, field: "usage", text: v })))),
        h(
          "div",
          { className: "lx-domains" },
          ...chips,
          keyed(`adddom:${s.id}`, h("button", {
            type: "button",
            className: "lx-add-domain",
            textContent: open ? "Close" : "+ Semantic domain",
            onclick: () => {
              picker = open ? null : { senseId: s.id, query: "", active: 0 };
              void ensureDomains().then(() => {
                drawEditor();
                el.querySelector<HTMLInputElement>(".lx-picker input")?.focus();
              }, (err) => showToast({ tone: "error", lines: [errorText(err)] }));
            },
          })),
        ),
        open ? pickerBlock(s) : null,
        h(
          "div",
          { className: "lx-sense-foot" },
          h("span", { className: "note", textContent: usageLabel(s.occurrences) }),
          h("span", { className: "gl-spacer" }),
          e.senses.length > 1 ? confirmButton(`dels:${s.id}`, "Delete sense", () => void run(() => lexiconDeleteSense({ senseId: s.id }), true)) : null,
        ),
      );
    }

    function pickerBlock(s: SenseView) {
      const all = domains ?? [];
      if (all.length === 0) {
        return h(
          "div",
          { className: "lx-picker" },
          h("p", { className: "note", textContent: "This project has no semantic domains yet." }),
          h("button", {
            type: "button",
            textContent: "Load SIL Semantic Domains",
            onclick: () =>
              void (async () => {
                try {
                  const r = await domainsLoadSil();
                  if (r.notes.length > 0) showToast({ tone: "warn", lines: r.notes });
                  showToast({ tone: "info", lines: [`Loaded ${r.created} semantic domains.`] });
                  await ensureDomains(true);
                } catch (err) {
                  showToast({ tone: "error", lines: [errorText(err)] });
                }
                drawEditor();
                el.querySelector<HTMLInputElement>(".lx-picker input")?.focus();
              })(),
          }),
          h("p", { className: "note lx-attr", textContent: `${ATTRIBUTION}. About 1,800 domains for collecting words by meaning.` }),
        );
      }
      const p = picker!;
      const have = new Set(s.domains.map((d) => d.id));
      const hits = filterDomains(all, p.query, 40);
      p.active = Math.min(p.active, Math.max(0, hits.length - 1));
      const assign = (d: DomainItem) => {
        picker = null;
        // Focus goes back to the sense's "+ Semantic domain", ready for the next one.
        el.querySelector<HTMLElement>(`[data-key="adddom:${CSS.escape(s.id)}"]`)?.focus();
        void run(() => lexiconAssignDomain({ senseId: s.id, domainId: d.id }));
      };
      const input = h("input", { type: "search", value: p.query, placeholder: "Type a word or a number (e.g. moon, 1.1.1)", ariaLabel: "Find a semantic domain" });
      input.dataset.key = `pick:${s.id}`;
      input.oninput = () => {
        p.query = input.value;
        p.active = 0;
        keepFocus(drawEditor);
      };
      input.onkeydown = (ev) => {
        if (ev.isComposing) return;
        if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
          ev.preventDefault();
          p.active = Math.max(0, Math.min(hits.length - 1, p.active + (ev.key === "ArrowDown" ? 1 : -1)));
          keepFocus(drawEditor);
        } else if (ev.key === "Enter") {
          ev.preventDefault();
          const d = hits[p.active];
          if (d !== undefined && !have.has(d.id)) assign(d);
        } else if (ev.key === "Escape") {
          picker = null;
          drawEditor();
        }
      };
      const options = hits.map((d, i) =>
        h(
          "li",
          {
            className: [i === p.active ? "on" : "", have.has(d.id) ? "taken" : ""].join(" ").trim(),
            title: d.description,
            onmousedown: (ev: MouseEvent) => ev.preventDefault(),
            onclick: () => {
              if (!have.has(d.id)) assign(d);
            },
          },
          h("span", { className: "lx-code", textContent: d.code }),
          h("span", { textContent: ` ${d.label}` }),
        ),
      );
      const active = hits[p.active];
      return h(
        "div",
        { className: "lx-picker" },
        input,
        h("ul", { className: "lx-options" }, ...(options.length === 0 ? [h("li", { className: "empty", textContent: "No domain matches." })] : options)),
        active === undefined || active.description === "" ? null : h("p", { className: "note lx-desc", textContent: active.description }),
        h("p", { className: "note lx-attr", textContent: ATTRIBUTION }),
      );
    }

    function drawAll() {
      drawList();
      drawEditor();
    }

    // ------------------------------------------------------------------ wiring

    let timer: number | undefined;
    search.oninput = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => void loadList(), 120);
    };
    search.onkeydown = (ev) => {
      if (ev.key !== "Enter") return;
      ev.preventDefault();
      // Enter can come before the debounce: search for what is typed now.
      window.clearTimeout(timer);
      void loadList().then(() => {
        // An entry spelled exactly as typed beats one that only matched a gloss.
        const typed = search.value.trim().normalize("NFC").toLocaleLowerCase();
        const pick = rows.find((r) => r.lemma.normalize("NFC").toLocaleLowerCase() === typed) ?? rows[0];
        if (pick !== undefined) void loadEntry(pick.id);
      });
    };
    addForm.onsubmit = (ev) => {
      ev.preventDefault();
      const lemma = newLemma.value.trim();
      if (lemma === "") {
        newLemma.focus();
        return;
      }
      const gloss = newGloss.value;
      newLemma.value = "";
      newGloss.value = "";
      selected = null;
      void run(() => lexiconAdd({ lemma, gloss }), true).then(() => newLemma.focus());
    };

    const view: PanelView = {
      element: el,
      render() {
        const s = getState();
        const key = s.kind === "open" ? `${s.info.path}|${dataGeneration()}|${lexiconRevision()}` : "closed";
        if (key === shownFor) return;
        const sameProject = s.kind === "open" && shownFor.split("|")[0] === s.info.path;
        // Someone is typing in the editor: refresh once they leave the field.
        if (sameProject && editing()) {
          if (!refreshWaiting) {
            refreshWaiting = true;
            el.addEventListener("focusout", () => window.setTimeout(() => {
              refreshWaiting = false;
              view.render();
            }), { once: true });
          }
          return;
        }
        shownFor = key;
        if (!sameProject) projectGen += 1;
        if (s.kind !== "open") {
          quietly(() => el.replaceChildren(emptyState("Lexicon", "Open a project to see its lexicon.", "The wug is still learning its first words.")));
          return;
        }
        if (!sameProject) {
          rows = [];
          selected = null;
          entry = null;
          paradigm = null;
          domains = null;
          picker = null;
          search.value = "";
          quietly(() => el.replaceChildren(search, addForm, body));
        } else {
          // Another device's ops or another panel's edit: domains may have
          // arrived too. An open picker stays open on the same sense.
          domains = null;
          if (picker !== null) void ensureDomains();
        }
        void loadList().then(() => loadEntry(selected, true));
      },
    };
    return view;
  },
};
