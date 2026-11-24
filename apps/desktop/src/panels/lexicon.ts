// The Lexicon panel (roadmap item 22): browse and edit entries, their senses,
// glosses in a second language, definitions, and semantic domains. Glossing
// grows the lexicon (the Gloss panel's add-to-lexicon); this is where it is
// tidied. Every edit returns the entry's fresh view, so the editor redraws
// from one answer; warnings (a duplicate lemma) arrive as a toast. The
// entry's paradigm (roadmap item 50) is its own view, fetched for the chosen
// template and returned whole by every form edit.
import {
  concordance, domainsList, domainsLoadSil, errorText, lexiconAdd, lexiconAddAllomorph, lexiconAddSense, lexiconClearPicture, lexiconSetPicture, lexiconDeleteAllomorph, lexiconAssignDomain, lexiconDelete, lexiconDeleteSense, lexiconEntry,
  deleteParadigmTemplate, lexiconDeleteForm, lexiconFillCell, lexiconMerge, paradigmTemplates, saveParadigmTemplate, lexiconList, lexiconNumberHomographs, lexiconParadigm, lexiconSetField, lexiconSetHomograph, lexiconSetSenseGloss,
  lexiconSetSenseText, lexiconUnassignDomain, lexiconAddExample, lexiconExampleFrom, lexiconSetExample, lexiconDeleteExample,
  lexiconAddPronunciation, lexiconSetPronunciation, lexiconPronunciationAudio, lexiconDeletePronunciation, lexiconAddRelation, lexiconDeleteRelation,
  saveDocument,
  type DomainItem, type PronunciationView, type ExampleView, type EntryEdit, type EntryRow, type EntryView, type LexemeField, type ParadigmEdit, type ParadigmForm, type ParadigmView, type SenseView,
} from "../api";
import { emptyState, h } from "../dom";
import { typeWithShortcuts } from "../typing";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { normalizeLangTag } from "../gloss/lang";
import { categoriesOf, filterDomains, glossLine, headword, LIST_SHOWS, showLabel, shows, type ListShow, nextHomograph, parseHomograph, RELATIONS, relatedName, relationLabel, sameLemmaNote, usageLabel } from "../lexicon/logic";
import { attestedForms, attestedLine, axesOf, cellFeatures, cellKey, cellLabel, coverageLine, draftOf, isOwn, layout, otherFormLine, paradigmClipboard, paradigmSheet, TEMPLATES, templateById, templateFromDraft, type Features, type Template, type TemplateDraft } from "../lexicon/paradigms";
import { dataGeneration, getSelection, getState, lexiconRevision, selectSegment, selectText, showToast, takeWantedEntry } from "../store";
import { hear, hearSpan, playButton } from "../hear/hear";
import { keyword, type Hit } from "../concordance/logic";
import { EXAMPLE_FORMATS, FORMAT_KEY, glossRuns, parseFormat } from "../gloss/example";
import { writeHtml, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

const SELECT_FIRST = "Select the segment where the word is said (on the Timeline or in Transcribe), then try again.";

/** Shared with the Gloss panel: the second analysis language. */
const LANG_KEY = "wugs.gloss-lang";
/** The paradigm template last chosen on this machine. */
const PARADIGM_KEY = "wugs.lexicon.paradigm";
/** The template picker's choice that opens the form for a new one. */
const NEW_TEMPLATE = "\u0000new";
const ATTRIBUTION = "SIL Semantic Domains v4 · © SIL Global · semdom.org · licensed CC BY-SA 4.0 (creativecommons.org/licenses/by-sa/4.0)";

/** Occurrences listed under a sense; the Concordance has the rest. */
const USES_SHOWN = 12;

export const lexiconPanel: PanelDef = {
  title: "Lexicon",
  create(): PanelView {
    const el = h("div", { className: "panel panel-lexicon" });
    const search = h("input", { type: "search", placeholder: "Find an entry by lemma or gloss" });
    /** Which entries the list shows: every one, or those with work left. */
    let show: ListShow = "all";
    const showPicker = h("select", { className: "lx-show", ariaLabel: "Which entries to list" });
    const searchBar = h("div", { className: "lx-search" }, search, showPicker);
    const newLemma = typeWithShortcuts(h("input", { placeholder: "Lemma", ariaLabel: "New entry lemma" }));
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
    /** The entry's word forms in the texts, asked for under the paradigm. */
    let attested: { lexeme: string; lines: Hit[] | null; total: number } | null = null;
    /** The project's own paradigm templates (shared through the project). */
    let ownTemplates: Template[] = [];
    /** The template form, while it is open: the template being edited (null
     * for a new one), what is typed, and what is missing in words. */
    let templateForm: { editing: Template | null; draft: TemplateDraft; error: string | null } | null = null;
    /** The sense whose domain picker is open, and what was typed in it. */
    let picker: { senseId: string; query: string; active: number } | null = null;
    /** The sense whose occurrences are listed (roadmap 72), and the hits once they came. */
    let uses: { senseId: string; hits: Hit[] | null; total: number } | null = null;
    /** An armed destructive button: its key; a second click carries it out. */
    let armed: string | null = null;
    /** The relation type the Related row adds next. */
    let relationKind = "synonym";
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

    /** Fetch the project's own templates; the one chosen on this machine
     * last time is taken up once it is among them. */
    async function loadOwnTemplates() {
      const gen = projectGen;
      try {
        const own = await paradigmTemplates();
        if (gen !== projectGen) return;
        ownTemplates = own;
      } catch {
        return;
      }
      // An own template changed or deleted elsewhere shows the grid it now
      // describes (or the first built-in one); the one chosen here last
      // time is taken up once the project's templates have arrived.
      const wanted = localStorage.getItem(PARADIGM_KEY);
      let next = template;
      if (isOwn(template)) next = ownTemplates.find((t) => t.id === template.id) ?? TEMPLATES[0]!;
      else if (wanted !== null && wanted !== template.id && ownTemplates.some((t) => t.id === wanted)) next = templateById(wanted, ownTemplates);
      if (JSON.stringify(next) !== JSON.stringify(template)) {
        template = next;
        paradigm = null;
        void loadParadigm();
      }
      keepFocus(drawEditor);
    }

    function useTemplate(t: Template) {
      template = t;
      localStorage.setItem(PARADIGM_KEY, t.id);
      paradigm = null;
      keepFocus(drawEditor);
      void loadParadigm();
    }

    async function saveTemplate() {
      const form = templateForm;
      if (form === null) return;
      const taken = ownTemplates.map((t) => t.id);
      const made = templateFromDraft(form.draft, form.editing?.id ?? null, taken);
      if (typeof made === "string") {
        form.error = made;
        keepFocus(drawEditor);
        return;
      }
      try {
        ownTemplates = await saveParadigmTemplate({ template: made });
      } catch (e) {
        form.error = errorText(e);
        keepFocus(drawEditor);
        return;
      }
      templateForm = null;
      useTemplate(ownTemplates.find((t) => t.id === made.id) ?? made);
    }

    async function deleteTemplate(t: Template) {
      try {
        ownTemplates = await deleteParadigmTemplate({ id: t.id });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
        return;
      }
      templateForm = null;
      showToast({ tone: "info", lines: [`Removed the template «${t.label}». Its forms stay in the lexicon.`] });
      useTemplate(TEMPLATES[0]!);
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
      // Headwords and other forms are typed in the language: they take its shortcuts.
      if (/^(lemma|cit|allo|prn|prn-new|exs):/u.test(key)) typeWithShortcuts(input);
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

    /** A multi-line field (definition, usage, notes): saves when it loses focus, or on Ctrl/⌘+Enter. */
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

    function drawShowPicker() {
      const current = LIST_SHOWS.find((x) => x.id === show) ?? LIST_SHOWS[0]!;
      showPicker.title = current.title;
      const cats = categoriesOf(rows, show);
      if (show.startsWith("pos:")) showPicker.title = `Entries whose category is «${show.slice(4)}»`;
      showPicker.replaceChildren(
        ...LIST_SHOWS.map((x) => h("option", { value: x.id, textContent: showLabel(rows, x), title: x.title, selected: x.id === show })),
        ...(cats.length === 0
          ? []
          : [h("optgroup", { label: "Category" }, ...cats.map((c) => h("option", { value: `pos:${c.category}`, textContent: `${c.category} (${c.count.toLocaleString("en")})`, selected: `pos:${c.category}` === show })))]),
      );
    }

    function drawList() {
      drawShowPicker();
      const listed = rows.filter((r) => shows(r, show));
      if (rows.length > 0 && listed.length === 0) {
        list.replaceChildren(h("li", { className: "empty", textContent: show.startsWith("pos:") ? `No entry found has the category «${show.slice(4)}».` : (LIST_SHOWS.find((x) => x.id === show) ?? LIST_SHOWS[0]!).done }));
        return;
      }
      if (rows.length === 0) {
        list.replaceChildren(
          h("li", { className: "empty", textContent: search.value.trim() === "" ? "No entries yet. Add one above, or gloss a word and add it from there." : "No entries match." }),
        );
        return;
      }
      list.replaceChildren(
        ...listed.map((r) =>
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
        row("Pronunciation", pronunciationsBlock(e)),
        row(
          "Other forms",
          h(
            "div",
            { className: "lx-forms" },
            ...e.allomorphs.map(([formId, form]) =>
              h(
                "span",
                { className: "lx-chip", title: "Another spelling of this entry (an allomorph): glossing finds it like the lemma" },
                `${form} `,
                h("button", {
                  type: "button",
                  className: "lx-x",
                  ariaLabel: `Remove ${form}`,
                  textContent: "×",
                  onclick: () => void run(() => lexiconDeleteAllomorph({ id: e.id, formId }), true),
                }),
              ),
            ),
            cell(`allo:${e.id}`, "", "Add another form", (v) => (v.trim() === "" ? Promise.resolve() : run(() => lexiconAddAllomorph({ id: e.id, form: v }), true)), {
              placeholder: e.allomorphs.length === 0 ? "allomorphs: -lar, ew…" : "add another",
              size: 14,
            }),
          ),
        ),
        row("Related", relationsBlock(e)),
        row("Homograph", homograph),
        row("Notes", area(`notes:${e.id}`, e.notes, "Notes on this entry: etymology, doubts, what to ask", (v) => run(() => lexiconSetField({ id: e.id, field: "notes", value: v })))),
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
              // A duplicate (two imports of one word): fold the other into this one.
              ...e.sameLemma.map(([other, n]) =>
                confirmButton(`merge:${other}`, `Merge ${n === null ? "the other" : headword(e.lemma, n)} into this one`, () => void run(() => lexiconMerge({ keep: e.id, other }), true), {
                  armedText: "Merge? Click again",
                  title: `Move the other «${headword(e.lemma, n)}»'s senses, forms, pronunciations and relations into this entry, then remove it. Glossed words linked to its senses stay linked.`,
                }),
              ),
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

    /** Synonyms, antonyms and the like: each a chip that opens the other
     * entry, and a type and a headword to add one. */
    function relationsBlock(e: EntryView) {
      const chips = e.relations.map((r) =>
        h(
          "span",
          { className: "lx-chip lx-rel" },
          h("span", { className: "lx-code", textContent: `${relationLabel(r.relation)}${r.hereSense === null ? "" : ` (${r.hereSense})`}` }),
          " ",
          h("button", { type: "button", className: "link-button", textContent: relatedName(r), title: "Open this entry", onclick: () => void loadEntry(r.otherId) }),
          " ",
          h("button", {
            type: "button",
            className: "lx-x",
            ariaLabel: `Remove the relation to ${relatedName(r)}`,
            textContent: "×",
            onclick: () => void run(() => lexiconDeleteRelation({ id: e.id, relationId: r.id })),
          }),
        ),
      );
      const kind = h("select", { ariaLabel: "Relation type" }, ...RELATIONS.map((r) => h("option", { value: r.id, textContent: r.label, selected: r.id === relationKind })));
      kind.dataset.key = `relkind:${e.id}`;
      kind.onchange = () => {
        relationKind = kind.value;
      };
      const target = cell(`rel:${e.id}`, "", "Related headword", (v) => (v.trim() === "" ? Promise.resolve() : run(() => lexiconAddRelation({ id: e.id, relation: kind.value, target: v }), true)), {
        placeholder: "headword, e.g. tela2",
        size: 14,
      });
      return h("div", { className: "lx-forms" }, ...chips, kind, target);
    }

    /** How the entry is said: each pronunciation a field, its ▶ when it is
     * heard in a recording, and a way to hear it in the segment selected on
     * the Timeline (a word cut out by Auto-segment by word, say). */
    function pronunciationsBlock(e: EntryView) {
      const selection = (): string | null => {
        const sel = getSelection();
        if (sel === null) showToast({ tone: "info", lines: [SELECT_FIRST] });
        return sel?.segmentId ?? null;
      };
      const one = (x: PronunciationView) => {
        const del = confirmButton(`delp:${x.id}`, "×", () => void run(() => lexiconDeletePronunciation({ id: e.id, pronunciationId: x.id })), {
          armedText: "Delete?",
          title: "Delete this pronunciation",
        });
        del.tabIndex = -1;
        const audio = x.audio;
        return h(
          "span",
          { className: "lx-pron" },
          cell(`prn:${x.id}`, x.form, "Pronunciation", (v) => run(() => lexiconSetPronunciation({ id: e.id, pronunciationId: x.id, form: v })), {
            placeholder: audio === null ? "" : "how it is written",
            size: 12,
          }),
          audio === null
            ? h("button", {
                type: "button",
                className: "link-button",
                textContent: "+ sound",
                title: "Hear this pronunciation in the segment selected on the Timeline",
                onclick: () => {
                  const seg = selection();
                  if (seg !== null) void run(() => lexiconPronunciationAudio({ id: e.id, pronunciationId: x.id, segmentId: seg }));
                },
              })
            : playButton(`Hear «${x.form || e.lemma}» (${audio.label || "recording"})`, () => void hearSpan(audio.sourceId, audio.startMs, audio.endMs)),
          del,
        );
      };
      return h(
        "div",
        { className: "lx-forms" },
        ...e.pronunciations.map(one),
        cell(`prn-new:${e.id}`, "", "Add a pronunciation", (v) => (v.trim() === "" ? Promise.resolve() : run(() => lexiconAddPronunciation({ id: e.id, form: v, segmentId: null }))), {
          placeholder: e.pronunciations.length === 0 ? "IPA: ˈkani" : "add another",
          size: 12,
        }),
        h("button", {
          type: "button",
          textContent: "From the selected segment",
          title: "Add a pronunciation heard in the segment selected on the Timeline; a one-word transcription becomes its written form",
          onclick: () => {
            const seg = selection();
            if (seg !== null) void run(() => lexiconAddPronunciation({ id: e.id, form: "", segmentId: seg }));
          },
        }),
      );
    }

    function paradigmBlock(e: EntryView) {
      const t = template;
      const option = (x: Template) => h("option", { value: x.id, textContent: x.label, selected: x.id === t.id });
      const select = h(
        "select",
        { ariaLabel: "Paradigm template" },
        ...TEMPLATES.map(option),
        ownTemplates.length === 0 ? null : h("optgroup", { label: "This project" }, ...ownTemplates.map(option)),
        h("option", { value: NEW_TEMPLATE, textContent: "New template…" }),
      );
      select.dataset.key = `pdg-template:${e.id}`;
      select.onchange = () => {
        if (select.value === NEW_TEMPLATE) {
          templateForm = { editing: null, draft: draftOf(null), error: null };
          drawThenFocus("pdg-form:name");
          return;
        }
        templateForm = null;
        useTemplate(templateById(select.value, ownTemplates));
      };
      const pd = paradigm !== null && paradigm.lexeme === e.id ? paradigm : null;
      const own = isOwn(t);
      const head = h(
        "div",
        { className: "lx-pdg-head" },
        h("h2", { textContent: "Paradigm" }),
        select,
        own
          ? h("button", {
              type: "button",
              textContent: "Edit template",
              title: "Change this template's name, features or values; every project member sees the change",
              onclick: () => {
                templateForm = { editing: t, draft: draftOf(t), error: null };
                drawThenFocus("pdg-form:name");
              },
            })
          : null,
        own ? confirmButton(`pdg-del:${t.id}`, "Delete template", () => void deleteTemplate(t), { title: "Remove this template from the project; the forms stay in the lexicon" }) : null,
        h("span", { className: "note", textContent: pd === null ? "Loading…" : coverageLine(t, pd.cells) }),
        pd === null
          ? null
          : (() => {
              let format = parseFormat(null);
              try {
                format = parseFormat(localStorage.getItem(FORMAT_KEY));
              } catch {
                /* the default */
              }
              const label = EXAMPLE_FORMATS.find((f) => f.id === format)?.label ?? "";
              return h("button", {
                type: "button",
                textContent: "Copy table",
                title: `Copy this paradigm as a table for a paper, as ${label} (the format chosen for Copy example in the Gloss panel)`,
                onclick: () => {
                  const out = paradigmClipboard(t, pd.cells, headword(e.lemma, e.homograph), format);
                  void (out.html === null ? writeText(out.text) : writeHtml(out.html, out.text)).then(
                    () => showToast({ tone: "info", lines: [`Copied the paradigm as ${label}.`] }),
                    (err) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(err)}`] }),
                  );
                },
              });
            })(),
        pd === null
          ? null
          : h("button", {
              type: "button",
              textContent: "Print a sheet…",
              title: "Save this paradigm as a form to print for a session: the grid with the forms known so far in grey, and lines for the speaker, date and place",
              onclick: () =>
                void (async () => {
                  const folder = await pickFile({ directory: true, title: "Where should the sheet go?" });
                  if (typeof folder !== "string") return;
                  const word = headword(e.lemma, e.homograph);
                  try {
                    const r = await saveDocument({ folder, title: `${word} ${t.label}`, extension: "html", contents: paradigmSheet(t, pd.cells, word, e.senses[0]?.gloss ?? "", true) });
                    showToast({ tone: "info", lines: [`Saved the sheet as ${r.path}. Open it in a browser and print it.`] });
                  } catch (err) {
                    showToast({ tone: "error", lines: [errorText(err)] });
                  }
                })(),
            }),
      );
      const form = templateForm === null ? null : templateFormBlock(templateForm);
      if (pd === null) return h("section", { className: "lx-paradigm" }, head, form);
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
      return h("section", { className: "lx-paradigm" }, head, form, table, others, attestedBlock(e, pd));
    }

    /** The word forms of the entry its glossed words show, to fill the grid from. */
    function attestedBlock(e: EntryView, pd: ParadigmView) {
      const at = attested?.lexeme === e.id ? attested : null;
      if (at === null) {
        const b = h("button", {
          type: "button",
          className: "link-button",
          textContent: "Forms in the texts",
          title: "List the word forms of this entry that glossed words in the texts show, to fill the grid from",
          onclick: () => void loadAttested(e),
        });
        b.dataset.key = `pdg-attested:${e.id}`;
        return h("div", { className: "lx-pdg-attested" }, b);
      }
      if (at.lines === null) return h("p", { className: "note", textContent: "Finding the entry's forms in the texts…" });
      const held = [...pd.cells.flatMap((c) => c.forms), ...pd.others].map((f) => f.form);
      const forms = attestedForms(at.lines, held);
      return h(
        "div",
        { className: "lx-pdg-attested" },
        h("p", { className: "note", textContent: attestedLine(forms, at.lines.length, at.total) }),
        forms.length === 0
          ? null
          : h(
              "ul",
              {},
              ...forms.slice(0, 60).map((f) =>
                h(
                  "li",
                  { className: f.inGrid ? "lx-pdg-held" : "" },
                  h("button", {
                    type: "button",
                    className: "link-button",
                    textContent: f.form,
                    title: `Show every «${f.form}» in the Concordance`,
                    onclick: () => askConcordance(f.form, "word", null, true),
                  }),
                  f.breakdown !== null && f.breakdown !== f.form ? h("span", { textContent: ` ${f.breakdown}` }) : null,
                  f.glosses !== null ? h("span", { className: "note" }, " ", ...glossRuns(f.glosses).map((r) => (r.caps ? h("span", { className: "sc", textContent: r.text }) : document.createTextNode(r.text)))) : null,
                  h("span", { className: "note", textContent: ` ×${f.count}${f.inGrid ? " · in the paradigm" : ""}` }),
                ),
              ),
            ),
      );
    }

    async function loadAttested(e: EntryView) {
      attested = { lexeme: e.id, lines: null, total: 0 };
      drawEditor();
      const gen = projectGen;
      try {
        const found = await Promise.all(e.senses.map((s) => concordance({ query: s.id, kind: "sense" })));
        if (gen !== projectGen || attested?.lexeme !== e.id) return;
        // A word with morphs linked to two senses of the entry is one use.
        const seen = new Set<string>();
        const lines = found.flatMap((r) => r.hits).filter((x) => {
          const k = `${x.segmentId} ${x.at}`;
          return !seen.has(k) && seen.add(k) !== undefined;
        });
        const cut = found.some((r) => r.total > r.hits.length);
        attested = { lexeme: e.id, lines, total: cut ? found.reduce((n, r) => n + r.total, 0) : lines.length };
      } catch (err) {
        if (attested?.lexeme === e.id) attested = null;
        showToast({ tone: "error", lines: [errorText(err)] });
      }
      drawEditor();
    }

    /** The template form: a name, the rows' feature and values, and
     * optionally the columns'. Drafts live in `templateForm`, so a redraw
     * from another device's edit keeps what is typed. */
    function templateFormBlock(f: NonNullable<typeof templateForm>) {
      const field = (key: keyof TemplateDraft, label: string, placeholder: string) => {
        const input = h("input", { value: f.draft[key], placeholder, ariaLabel: label });
        input.dataset.key = `pdg-form:${key}`;
        input.oninput = () => {
          f.draft[key] = input.value;
        };
        input.onkeydown = (ev) => {
          if (ev.key === "Enter" && !ev.isComposing) {
            ev.preventDefault();
            void saveTemplate();
          } else if (ev.key === "Escape") {
            templateForm = null;
            keepFocus(drawEditor);
          }
        };
        return h("label", {}, h("span", { textContent: label }), input);
      };
      return h(
        "div",
        { className: "lx-pdg-template", role: "group", ariaLabel: f.editing === null ? "New paradigm template" : `Edit template ${f.editing.label}` },
        field("name", "Name", "Case × number"),
        field("rows", "Rows: feature", "Case"),
        field("rowValues", "Rows: values", "nom=nominative, acc=accusative, gen"),
        field("cols", "Columns: feature (optional)", "Number"),
        field("colValues", "Columns: values", "sg=singular, pl=plural"),
        h("p", {
          className: "note",
          textContent: "Values are separated by commas; write value=meaning to show the meaning in the grid. The template is kept with the project, so everyone working on it can use it.",
        }),
        f.error === null ? null : h("p", { className: "note error-text", role: "alert", textContent: f.error }),
        h(
          "div",
          { className: "inline" },
          h("button", { type: "button", textContent: f.editing === null ? "Add template" : "Save template", onclick: () => void saveTemplate() }),
          h("button", {
            type: "button",
            textContent: "Cancel",
            onclick: () => {
              templateForm = null;
              keepFocus(drawEditor);
            },
          }),
        ),
      );
    }

    function drawThenFocus(key: string) {
      quietly(drawEditor);
      const target = el.querySelector<HTMLElement>(`[data-key="${key}"]`);
      target?.focus();
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

    /** A picture of what the sense means, for picture dictionaries, the website and flashcards. */
    function pictureBlock(s: SenseView) {
      const choose = async () => {
        const path = await pickFile({ title: "Choose a picture of this meaning", multiple: false, directory: false, filters: [{ name: "Pictures", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }] });
        if (typeof path === "string") await run(() => lexiconSetPicture({ sense: s.id, path }));
      };
      const pic = s.picture ?? null;
      if (pic === null) {
        return h("div", { className: "lx-picture" }, keyed(`pic:${s.id}`, h("button", { type: "button", className: "lx-add-domain", textContent: "+ Picture", title: "A photo or drawing of what this means: shown here, in the printed dictionary, on the website and on flashcards", onclick: () => void choose() })));
      }
      return h(
        "div",
        { className: "lx-picture" },
        pic.path === null
          ? h("span", { className: "note", textContent: `${pic.name}: not on this computer yet` })
          : h("img", { src: convertFileSrc(pic.path), alt: s.gloss || pic.name, title: pic.name }),
        keyed(`pic:${s.id}`, h("button", { type: "button", className: "lx-add-domain", textContent: "Change…", onclick: () => void choose() })),
        confirmButton(`unpic:${s.id}`, "Remove picture", () => void run(() => lexiconClearPicture({ sense: s.id }))),
      );
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
        h("div", { className: "lx-def" }, area(`nsen:${s.id}`, s.notes, "Notes on this meaning", (v) => run(() => lexiconSetSenseText({ senseId: s.id, field: "notes", text: v })))),
        examplesBlock(s),
        pictureBlock(s),
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
          s.occurrences === 0 ? h("span", { className: "note", textContent: usageLabel(0) }) : usesToggle(s),
          h("span", { className: "gl-spacer" }),
          e.senses.length > 1 ? confirmButton(`dels:${s.id}`, "Delete sense", () => void run(() => lexiconDeleteSense({ senseId: s.id }), true)) : null,
        ),
        uses?.senseId === s.id ? usesBlock(uses) : null,
      );
    }

    /** The sense's example sentences, each sentence and translation a field,
     * and a last pair of fields that adds one on Enter. */
    function examplesBlock(s: SenseView) {
      const one = (x: ExampleView, i: number) => {
        const from = x.from;
        return h(
          "li",
          { className: "lx-example" },
          h("span", { className: "lx-ex-n", textContent: `${i + 1}.` }),
          h(
            "div",
            { className: "lx-ex-body" },
            cell(`exs:${x.id}`, x.sentence, `Example ${i + 1}`, (v) => run(() => lexiconSetExample({ exampleId: x.id, field: "sentence", text: v })), { placeholder: "Example sentence" }),
            cell(`ext:${x.id}`, x.translation, `Translation of example ${i + 1}`, (v) => run(() => lexiconSetExample({ exampleId: x.id, field: "translation", text: v })), { placeholder: "Translation" }),
            from === null && x.reference === ""
              ? null
              : h(
                  "span",
                  { className: "note lx-ex-from" },
                  from === null
                    ? null
                    : h("button", { type: "button", className: "link-button", title: "Open the sentence it was taken from", textContent: `from «${from.textTitle || "Untitled text"}»`, onclick: () => void openSentence(from) }),
                  from !== null && x.reference !== "" ? " · " : "",
                  x.reference,
                ),
          ),
          from !== null && from.startMs !== null && from.endMs !== null
            ? playButton(`Play the sentence this example was taken from`, () => void hear({ textId: from.textId, tierId: from.tierId, segmentId: from.segmentId, startMs: from.startMs, endMs: from.endMs }))
            : null,
          confirmButton(`delx:${x.id}`, "×", () => void run(() => lexiconDeleteExample({ exampleId: x.id })), { armedText: "Delete?", title: "Delete this example" }),
        );
      };
      const sentence = h("input", { placeholder: s.examples.length === 0 ? "Add an example sentence" : "Another example", ariaLabel: "New example sentence" });
      const translation = h("input", { placeholder: "Its translation", ariaLabel: "Translation of the new example" });
      sentence.dataset.key = `newx:${s.id}`;
      translation.dataset.key = `newxt:${s.id}`;
      sentence.dataset.saved = "";
      translation.dataset.saved = "";
      const add = (refocus: boolean) => {
        if (drawing || sentence.value.trim() === "") return;
        const [sv, tv] = [sentence.value, translation.value];
        sentence.value = "";
        translation.value = "";
        if (refocus) sentence.focus();
        void run(() => lexiconAddExample({ senseId: s.id, sentence: sv, translation: tv }));
      };
      const onKey = (ev: KeyboardEvent) => {
        if (ev.key !== "Enter" || ev.isComposing) return;
        ev.preventDefault();
        add(true);
      };
      // Leaving the pair keeps what was typed as an example, like every other field here.
      const onBlur = (ev: FocusEvent) => {
        if (ev.relatedTarget !== sentence && ev.relatedTarget !== translation) add(false);
      };
      sentence.onkeydown = onKey;
      translation.onkeydown = onKey;
      sentence.onblur = onBlur;
      translation.onblur = onBlur;
      return h(
        "div",
        { className: "lx-examples" },
        h("span", { className: "lx-ex-label", textContent: "Examples" }),
        s.examples.length === 0 ? null : h("ol", { className: "lx-example-list" }, ...s.examples.map(one)),
        h("div", { className: "lx-ex-new" }, sentence, translation),
      );
    }

    async function openSentence(from: NonNullable<ExampleView["from"]>) {
      await selectText(from.textId);
      const st = getState();
      if (st.kind === "open" && st.text?.id === from.textId) selectSegment({ textId: from.textId, tierId: from.tierId, segmentId: from.segmentId });
    }

    /** "used in 3 words" opens the list of those words, each one to hear. */
    function usesToggle(s: SenseView) {
      const shown = uses?.senseId === s.id;
      const b = h("button", { type: "button", className: "link-button", textContent: usageLabel(s.occurrences), title: shown ? "Hide where it is used" : "Show where it is used, to hear each" });
      b.dataset.key = `uses:${s.id}`;
      b.setAttribute("aria-expanded", String(shown));
      b.onclick = () => {
        if (shown) {
          uses = null;
          drawEditor();
          return;
        }
        void loadUses(s.id);
      };
      return b;
    }

    async function loadUses(senseId: string) {
      uses = { senseId, hits: null, total: 0 };
      drawEditor();
      const gen = projectGen;
      try {
        const r = await concordance({ query: senseId, kind: "sense" });
        if (gen !== projectGen || uses?.senseId !== senseId) return;
        uses = { senseId, hits: r.hits, total: r.total };
      } catch (e) {
        if (uses?.senseId === senseId) uses = null;
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      drawEditor();
    }

    function usesBlock(u: { senseId: string; hits: Hit[] | null; total: number }) {
      if (u.hits === null) return h("div", { className: "lx-uses note", textContent: "Finding where it is used…" });
      const shown = u.hits.slice(0, USES_SHOWN);
      return h(
        "ul",
        { className: "lx-uses" },
        ...shown.map((hit) =>
          h(
            "li",
            {},
            playButton(`Play «${keyword(hit)}» in «${hit.textTitle}»`, () => void hear(hit)),
            h("button", {
              type: "button",
              className: "link-button lx-use",
              title: "Open this segment",
              onclick: () => void openHit(hit),
            }, h("strong", { textContent: keyword(hit) }), hit.breakdown !== null && hit.breakdown !== keyword(hit) ? ` ${hit.breakdown}` : "", h("span", { className: "note", textContent: ` · ${hit.textTitle}` })),
            h("button", {
              type: "button",
              className: "link-button lx-use-example",
              textContent: "Use as example",
              title: "Copy this sentence and its translation into the sense's examples",
              onclick: () => void run(() => lexiconExampleFrom({ senseId: u.senseId, segmentId: hit.segmentId })),
            }),
          ),
        ),
        u.total > shown.length ? h("li", { className: "note", textContent: `and ${u.total - shown.length} more (Concordance lists them all)` }) : null,
      );
    }

    async function openHit(hit: Hit) {
      await selectText(hit.textId);
      const st = getState();
      if (st.kind === "open" && st.text?.id === hit.textId) selectSegment({ textId: hit.textId, tierId: hit.tierId, segmentId: hit.segmentId });
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
    showPicker.onchange = () => {
      show = showPicker.value as ListShow;
      quietly(drawList);
    };
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
          ownTemplates = [];
          templateForm = null;
          if (isOwn(template)) template = TEMPLATES[0]!;
          domains = null;
          picker = null;
          uses = null;
          search.value = "";
          show = "all";
          quietly(() => el.replaceChildren(searchBar, addForm, body));
        } else {
          // Another device's ops or another panel's edit: domains may have
          // arrived too. An open picker stays open on the same sense.
          domains = null;
          if (picker !== null) void ensureDomains();
          if (uses !== null) void loadUses(uses.senseId);
        }
        void loadOwnTemplates();
        const wanted = takeWantedEntry();
        if (wanted !== null) {
          search.value = "";
          show = "all";
        }
        void loadList().then(() => loadEntry(wanted ?? selected, wanted === null));
      },
    };
    return view;
  },
};
