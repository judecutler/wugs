// The Elicit panel (roadmap item 23): work through a word list (Swadesh,
// Leipzig-Jakarta) or the semantic domains' questions with a speaker, typing
// each answer as it comes. Every answer is a lexicon entry, linked to the
// concept or assigned to the domain, so the lexicon grows as the list fills.
// Keyboard first: Enter saves the answer and moves to the next concept to do.
import {
  conceptListLoad, conceptLists, conceptListsBundled, conceptListView, domainCollect, domainQuestions, domainsList, domainsLoadSil,
  conceptListImport, elicitAnswer, elicitLink, elicitSkip, elicitUnlink, errorText, exportWordlist, lexiconEntry, lexiconList, projectLanguage, saveDocument,
  type Answer, type BundledList, type ConceptView, type DomainItem, type DomainQuestions, type EntryRow, type ListSummary, type ListView,
} from "../api";
import { emptyState, h } from "../dom";
import { typeWithShortcuts } from "../typing";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { localDay } from "../dates";
import { exactMatches, FILTERS, filterConcepts, isDone, nextOnTrack, nextTodo, progressLabel, progressShare, ownListLine, soundLine, soundSegment, wordlistLines, coverageLine, nextEmptyDomain, type Filter } from "../elicit/logic";
import { hear, hearSpan, playButton } from "../hear/hear";
import { elicitationSheet } from "../elicit/sheet";
import { clockTime } from "../search/logic";
import { filterDomains, headword } from "../lexicon/logic";
import { dataGeneration, getSelection, getState, lexiconRevision, showToast, subscribeSelection, touchLexicon } from "../store";
import type { PanelDef, PanelView } from "./panel";

const CONCEPTICON = "Concept lists from Concepticon (concepticon.clld.org) · CC BY 4.0";
const SIL = "SIL Semantic Domains v4 · © SIL Global · CC BY-SA 4.0 · semdom.org";
const LIST_KEY = "wugs.elicit-list";
const SOUND_KEY = "wugs.elicit.takeSound";

type Mode = "lists" | "domains";

export const elicitPanel: PanelDef = {
  title: "Elicit",
  create(): PanelView {
    const el = h("div", { className: "panel panel-elicit" });
    let mode: Mode = "lists";
    let shownFor = "";

    // word lists
    let lists: ListSummary[] = [];
    let bundled: BundledList[] = [];
    let listId: string | null = localStorage.getItem(LIST_KEY);
    let view: ListView | null = null;
    let filter: Filter = "todo";
    let current: string | null = null;
    let offers: EntryRow[] = [];
    /** The concept `offers` were looked up for. */
    let offersFor: string | null = null;
    let offerSeq = 0;
    /** What was typed per concept and not saved yet; survives redraws. */
    const drafts = new Map<string, { form: string; gloss: string }>();
    /** Concepts whose answer, link or N/A is still on its way: done for Enter's purposes. */
    const pending = new Set<string>();
    /** An IME or dead-key composition is open: a redraw now would cut it off. */
    let composing = false;
    let redrawAfterComposition = false;
    el.addEventListener("compositionstart", () => {
      composing = true;
    });
    el.addEventListener("compositionend", () => {
      composing = false;
      if (redrawAfterComposition) {
        redrawAfterComposition = false;
        window.setTimeout(() => keepFocus());
      }
    });
    let addingList = false;
    /** Answers take their sound from the Timeline's selected segment, and
     * the next segment on its track is selected and played (a word list
     * recorded first, then typed in as it is heard back). */
    let takeSound = localStorage.getItem(SOUND_KEY) === "on";
    const soundHint = h("p", { className: "note el-sound-hint" });
    /** The selected segment the next answer would take its sound from. */
    const soundNow = () => {
      const s = getState();
      return soundSegment(getSelection(), s.kind === "open" ? s.text : null);
    };
    const showSoundHint = () => {
      soundHint.textContent = takeSound ? soundLine(soundNow(), clockTime) : "";
    };
    const unwatchSelection = subscribeSelection(showSoundHint);
    /** Take the selected segment for an answer, and move the Timeline on to the next one. */
    const takeSegment = (): string | null => {
      if (!takeSound) return null;
      const seg = soundNow();
      const s = getState();
      if (seg === null || s.kind !== "open" || s.text === null) return null;
      const next = nextOnTrack(s.text.segments, seg);
      if (next !== null) void hear({ textId: s.text.id, tierId: next.tier, segmentId: next.id, startMs: next.startMs, endMs: next.endMs });
      return seg.id;
    };

    // domains
    let domains: DomainItem[] | null = null;
    let domainQuery = "";
    let emptyOnly = false;
    let domain: DomainQuestions | null = null;

    let queue: Promise<unknown> = Promise.resolve();
    let refreshWaiting = false;
    /** Bumped on every project switch: a job queued for the old project is dropped. */
    let projectGen = 0;

    /** Run edits one after another. A job gets `stale()`, true once the
     * project changed under it, to check after each await. */
    function enqueue(job: (stale: () => boolean) => Promise<void>) {
      const gen = projectGen;
      const stale = () => gen !== projectGen;
      const next = queue.then(async () => {
        if (stale()) return;
        try {
          await job(stale);
        } catch (e) {
          showToast({ tone: "error", lines: [errorText(e)] });
        }
      });
      queue = next;
      return next;
    }

    const notes = (n: string[]) => {
      if (n.length > 0) showToast({ tone: "warn", lines: n });
    };

    // ------------------------------------------------------------------ word lists

    async function loadLists() {
      [lists, bundled] = await Promise.all([conceptLists(), conceptListsBundled()]);
      if (listId !== null && !lists.some((l) => l.id === listId)) listId = null;
      if (listId === null && lists.length > 0) listId = lists[0]!.id;
      view = listId === null ? null : await conceptListView({ listId });
      if (view !== null && (current === null || !view.concepts.some((c) => c.id === current))) current = nextTodo(view.concepts, null)?.id ?? null;
    }

    function replaceConcept(c: ConceptView) {
      if (view === null) return;
      const i = view.concepts.findIndex((x) => x.id === c.id);
      if (i < 0) return;
      view.concepts[i] = c;
      const s = view.summary;
      s.answered = view.concepts.filter((x) => x.answers.length > 0).length;
      s.skipped = view.concepts.filter((x) => x.answers.length === 0 && x.skipped).length;
    }

    function focusCurrent() {
      if (current === null) return;
      const input = el.querySelector<HTMLInputElement>(`input[data-concept="${CSS.escape(current)}"]`);
      input?.focus();
      input?.scrollIntoView({ block: "nearest" });
    }

    /** Redraw, keeping the focused field (by its data-key) focused with its caret. */
    function keepFocus() {
      if (composing) {
        redrawAfterComposition = true;
        return;
      }
      const a = document.activeElement;
      const key = a instanceof HTMLInputElement && el.contains(a) ? a.dataset.key : undefined;
      const sel = a instanceof HTMLInputElement ? [a.selectionStart, a.selectionEnd] as const : null;
      draw();
      if (key === undefined) return;
      const again = el.querySelector<HTMLInputElement>(`input[data-key="${CSS.escape(key)}"]`);
      if (again === null) return;
      again.focus();
      if (sel !== null && sel[0] !== null && sel[1] !== null) again.setSelectionRange(sel[0], sel[1]);
    }

    /** The concept after `id` that Enter moves to: the next to-do, or under
     * Done / All the next row shown, so focus never lands on a hidden row. */
    function following(id: string): string | null {
      const all = view?.concepts ?? [];
      if (filter === "todo") {
        // A concept whose save is on its way counts as done, so Enter never wraps back onto it.
        const open = all.filter((c) => c.id === id || !pending.has(c.id));
        const next = nextTodo(open, id);
        return next === null || next.id === id || pending.has(next.id) ? null : next.id;
      }
      const shown = filterConcepts(all, filter);
      const i = shown.findIndex((c) => c.id === id);
      return shown[i + 1]?.id ?? shown[i]?.id ?? null;
    }

    /** Move to another concept: its own draft comes with it, offers do not. */
    function moveTo(id: string | null) {
      if (id !== current) {
        offerSeq += 1;
        offers = [];
        offersFor = null;
      }
      current = id;
    }

    function answerChip(c: ConceptView, a: Answer) {
      return h(
        "span",
        { className: "lx-chip", title: `${a.lemma} – ${a.gloss}` },
        ...(a.sound === null ? [] : [playButton(`Hear ${a.lemma}`, () => void hearSpan(a.sound!.sourceId, a.sound!.startMs, a.sound!.endMs))]),
        h("strong", { textContent: headword(a.lemma, a.homograph) }),
        a.gloss === "" || a.gloss === c.gloss ? "" : ` ‘${a.gloss}’`,
        h("button", {
          type: "button",
          className: "lx-x",
          ariaLabel: `Take back ${a.lemma}`,
          title: "Not an answer to this concept (the entry stays in the lexicon)",
          textContent: "×",
          onclick: () =>
            void enqueue(async (stale) => {
              const r = await elicitUnlink({ conceptId: c.id, senseId: a.senseId });
              if (stale()) return;
              replaceConcept(r.concept);
              notes(r.notes);
              keepFocus();
            }),
        }),
      );
    }

    /** Export this list's answers for comparative work: a CLDF Wordlist and an EDICTOR table. */
    function exportButton(listId: string): HTMLElement {
      const b = h("button", { type: "button", textContent: "Export for comparison…", title: "This list's answers as a CLDF Wordlist (Lexibank, CLICS) and a table EDICTOR and LingPy open, each form cut into sound segments" });
      b.onclick = async () => {
        const folder = await pickFolder({ directory: true, title: "Where should the word list go?" });
        if (typeof folder !== "string") return;
        try {
          const r = await exportWordlist({ folder, date: localDay(new Date()), listId });
          showToast({ tone: r.forms === 0 ? "warn" : "info", lines: wordlistLines(r) });
        } catch (e) {
          showToast({ tone: "error", lines: [errorText(e)] });
        }
      };
      return b;
    }

    /** This list on paper for a session without a laptop: the concepts the
     * filter shows (still to ask, or all), answers already here in grey. */
    function sheetButton(): HTMLElement {
      const b = h("button", {
        type: "button",
        textContent: "Print a sheet…",
        title: "This list as a form to print and fill in by hand: a header for the speaker, date and place, then a row per concept with room for the word and notes. With To do chosen, only the concepts still to ask",
      });
      b.onclick = async () => {
        const v = view;
        if (v === null) return;
        const folder = await pickFolder({ directory: true, title: "Where should the sheet go?" });
        if (typeof folder !== "string") return;
        try {
          const language = await projectLanguage().catch(() => ({ name: "", glottocode: "", iso: "" }));
          const which = filter === "todo" ? "todo" : "all";
          const contents = elicitationSheet({
            list: { name: v.summary.name, citation: v.summary.citation },
            language: language.name,
            concepts: v.concepts.map((c) => ({ number: c.number, gloss: c.gloss, concepticonGloss: c.concepticonGloss, skipped: c.skipped, answers: c.answers.map((a) => ({ lemma: a.lemma, gloss: a.gloss })) })),
            which,
            withAnswers: which === "all",
            generator: "wugs",
          });
          const r = await saveDocument({ folder, title: `${v.summary.name} sheet`, extension: "html", contents });
          showToast({ tone: "info", lines: [`Saved a sheet to print in ${r.path}. Open it in a browser or Word and print it.`, which === "todo" ? "It holds the concepts still to ask; choose All first for every concept, with the answers already here in grey." : "Answers already in the project are printed in grey, to check them."] });
        } catch (e) {
          showToast({ tone: "error", lines: [errorText(e)] });
        }
      };
      return b;
    }

    function conceptRow(c: ConceptView) {
      const isCurrent = c.id === current;
      const draft = drafts.get(c.id) ?? { form: "", gloss: "" };
      const form = typeWithShortcuts(h("input", { value: draft.form, placeholder: isCurrent ? "Word…" : "", ariaLabel: `Word for ${c.gloss}` }));
      form.dataset.concept = c.id;
      form.dataset.key = `form:${c.id}`;
      const gloss = h("input", { value: draft.gloss, placeholder: c.gloss, ariaLabel: `Gloss for ${c.gloss}`, className: "el-gloss" });
      gloss.dataset.key = `gloss:${c.id}`;
      const remember = () => {
        if (form.value === "" && gloss.value === "") drafts.delete(c.id);
        else drafts.set(c.id, { form: form.value, gloss: gloss.value });
      };
      form.onfocus = () => {
        if (current !== c.id) {
          moveTo(c.id);
          for (const row of el.querySelectorAll(".el-row.current")) row.classList.remove("current");
          form.closest(".el-row")?.classList.add("current");
          drawOffers();
        }
      };
      gloss.oninput = remember;
      form.oninput = () => {
        remember();
        const mine = ++offerSeq;
        const typed = form.value;
        window.setTimeout(() => {
          if (mine !== offerSeq) return;
          void lexiconList({ query: typed.trim() }).then((rows) => {
            // A newer keystroke, an Enter or a move to another row makes this answer stale.
            if (mine !== offerSeq || current !== c.id) return;
            offers = exactMatches(rows, typed);
            offersFor = c.id;
            drawOffers();
          }, () => {});
        }, 150);
      };
      const save = () => {
        const typed = form.value.trim();
        const next = following(c.id);
        if (typed === "") {
          // Enter on an empty word: move on without answering; the gloss
          // field keeps whatever was typed in it.
          moveTo(next ?? c.id);
          draw();
          focusCurrent();
          return;
        }
        const g = gloss.value;
        const segmentId = takeSegment();
        drafts.delete(c.id);
        pending.add(c.id);
        // Move on now, before the save answers: the next word typed belongs to the next concept.
        moveTo(next);
        draw();
        focusCurrent();
        void enqueue(async (stale) => {
          try {
            const r = await elicitAnswer({ conceptId: c.id, form: typed, gloss: g, segmentId });
            if (stale()) return;
            replaceConcept(r.concept);
            notes(r.notes);
            touchLexicon();
          } catch (e) {
            // Keep the word: it goes back into its field unless something newer was typed there.
            if (!stale() && !drafts.has(c.id)) drafts.set(c.id, { form: typed, gloss: g });
            throw e;
          } finally {
            pending.delete(c.id);
            if (!stale()) keepFocus();
          }
        });
      };
      for (const input of [form, gloss]) {
        input.onkeydown = (ev) => {
          if (ev.isComposing) return;
          if (ev.key === "Enter") {
            ev.preventDefault();
            save();
          } else if (ev.key === "Escape") {
            form.value = "";
            gloss.value = "";
            drafts.delete(c.id);
            offerSeq += 1;
            offers = [];
            offersFor = null;
            drawOffers();
          }
        };
      }
      const skip = h("button", {
        type: "button",
        className: "el-skip",
        textContent: c.skipped ? "Not applicable ✓" : "N/A",
        title: c.skipped ? `Marked not applicable${c.skipNote === "" ? "" : `: ${c.skipNote}`}. Click to undo.` : "Mark as not applicable to this language (anything typed in the gloss field is kept as the reason)",
        onclick: () => {
          // A reason, when one was typed into the gloss field (webview prompts are unreliable).
          const note = c.skipped ? "" : gloss.value;
          if (!c.skipped) {
            drafts.delete(c.id);
            pending.add(c.id);
            if (current === c.id || current === null) {
              moveTo(following(c.id));
              draw();
              focusCurrent();
            }
          }
          void enqueue(async (stale) => {
            try {
              const r = await elicitSkip({ conceptId: c.id, skipped: !c.skipped, note });
              if (stale()) return;
              replaceConcept(r.concept);
              notes(r.notes);
            } finally {
              pending.delete(c.id);
              if (!stale()) keepFocus();
            }
          });
        },
      });
      return h(
        "li",
        { className: ["el-row", isCurrent ? "current" : "", isDone(c) ? "done" : ""].join(" ").trim() },
        h("span", { className: "el-num", textContent: String(c.number) }),
        h("span", { className: "el-concept", textContent: c.gloss, title: c.concepticonGloss === "" ? "" : `Concepticon: ${c.concepticonGloss}` }),
        h("span", { className: "el-answers" }, ...c.answers.map((a) => answerChip(c, a))),
        h("span", { className: "el-entry" }, form, gloss, h("span", { className: "el-offers" })),
        skip,
      );
    }

    function drawOffers() {
      for (const box of el.querySelectorAll<HTMLElement>(".el-offers")) box.replaceChildren();
      // Offers belong to the concept they were looked up for, and only show there.
      const concept = offersFor;
      if (concept === null || concept !== current || offers.length === 0) return;
      const box = el.querySelector<HTMLElement>(`input[data-concept="${CSS.escape(concept)}"]`)?.parentElement?.querySelector<HTMLElement>(".el-offers");
      if (box === undefined || box === null) return;
      box.replaceChildren(
        ...offers.map((r) =>
          h("button", {
            type: "button",
            className: "gl-alt",
            title: "Link this existing entry instead of adding a new one",
            textContent: `Use ${headword(r.lemma, r.homograph)}${r.glosses[0] === undefined ? "" : ` ‘${r.glosses[0]}’`}`,
            onmousedown: (ev: MouseEvent) => ev.preventDefault(),
            onclick: () => {
              const segmentId = takeSegment();
              drafts.delete(concept);
              pending.add(concept);
              moveTo(following(concept));
              draw();
              focusCurrent();
              void enqueue(async (stale) => {
                try {
                  const entry = await lexiconEntry({ id: r.id });
                  const sense = entry.senses[0];
                  if (sense === undefined || stale()) return;
                  const res = await elicitLink({ conceptId: concept, senseId: sense.id, segmentId });
                  if (stale()) return;
                  replaceConcept(res.concept);
                  notes(res.notes);
                } finally {
                  pending.delete(concept);
                  if (!stale()) keepFocus();
                }
              });
            },
          }),
        ),
      );
    }

    function listsBody(): HTMLElement[] {
      const picker = h(
        "select",
        {
          ariaLabel: "Word list",
          onchange: () => {
            listId = picker.value;
            localStorage.setItem(LIST_KEY, listId);
            current = null;
            void enqueue(async () => {
              const v = await conceptListView({ listId: listId! });
              view = v;
              current = nextTodo(v.concepts, null)?.id ?? null;
              draw();
            });
          },
        },
        ...lists.map((l) => h("option", { value: l.id, textContent: l.name, selected: l.id === listId })),
      );
      const unloaded = bundled.filter((b) => b.listId === null);
      const add = h("button", { type: "button", textContent: addingList ? "Close" : "Add a word list", onclick: () => { addingList = !addingList; draw(); } });
      const own = h("div", { className: "el-bundled" },
        h("div", {}, h("strong", { textContent: "A list of your own" })),
        h("p", { className: "note", textContent: "A CSV or tab-separated table with a column of concepts (headed Gloss, English or Concept), and if you have them a number column and a Concepticon_ID column. The list takes the file's name." }),
        h("button", {
          type: "button",
          textContent: "Load a table…",
          onclick: async () => {
            const path = await pickFolder({ multiple: false, title: "Choose a word list table", filters: [{ name: "Tables", extensions: ["csv", "tsv", "txt", "tab", "xlsx", "xls", "ods"] }] });
            if (typeof path !== "string") return;
            void enqueue(async () => {
              try {
                const r = await conceptListImport({ path });
                listId = r.summary.id;
                localStorage.setItem(LIST_KEY, r.summary.id);
                addingList = false;
                current = null;
                showToast({ tone: "info", lines: [ownListLine(r)] });
                await loadLists();
                draw();
                focusCurrent();
              } catch (e) {
                showToast({ tone: "error", lines: [errorText(e)] });
              }
            });
          },
        }),
      );
      const adder =
        !addingList && lists.length > 0
          ? null
          : h(
              "div",
              { className: "lx-picker" },
              lists.length === 0 ? h("p", { textContent: "Pick a list to work through. Each answer becomes a lexicon entry linked to the concept." }) : null,
              ...unloaded.map((b) =>
                h(
                  "div",
                  { className: "el-bundled" },
                  h("div", {}, h("strong", { textContent: b.name }), h("span", { className: "note", textContent: ` ${b.count} concepts` })),
                  h("p", { className: "note", textContent: b.citation }),
                  h("button", {
                    type: "button",
                    textContent: "Load",
                    onclick: () =>
                      void enqueue(async () => {
                        const s = await conceptListLoad({ source: b.source });
                        listId = s.id;
                        localStorage.setItem(LIST_KEY, s.id);
                        addingList = false;
                        current = null;
                        await loadLists();
                        draw();
                        focusCurrent();
                      }),
                  }),
                ),
              ),
              own,
              h("p", { className: "note lx-attr", textContent: CONCEPTICON }),
            );
      if (view === null) return [h("div", { className: "el-bar" }, add), ...(adder === null ? [] : [adder])];
      const s = view.summary;
      const fill = h("div");
      fill.style.width = `${Math.round(progressShare(s) * 100)}%`;
      const bar = h("div", { className: "el-progress", title: progressLabel(s) }, fill);
      const chips = h(
        "div",
        { className: "scopes" },
        ...FILTERS.map(({ filter: f, label }) =>
          h("button", { type: "button", textContent: label, ariaPressed: String(f === filter), onclick: () => { filter = f; draw(); } }),
        ),
      );
      const shown = filterConcepts(view.concepts, filter);
      const rows = shown.length === 0
        ? [h("li", { className: "empty", textContent: filter === "todo" ? "Every concept is answered or marked not applicable. 🎉" : "Nothing here yet." })]
        : shown.map(conceptRow);
      const soundBox = h("input", { type: "checkbox", checked: takeSound });
      soundBox.onchange = () => {
        takeSound = soundBox.checked;
        localStorage.setItem(SOUND_KEY, takeSound ? "on" : "off");
        showSoundHint();
      };
      showSoundHint();
      return [
        h("div", { className: "el-bar" }, picker, h("span", { className: "note", textContent: progressLabel(s) }), h("span", { className: "gl-spacer" }), sheetButton(), exportButton(s.id), add),
        bar,
        ...(adder === null ? [] : [adder]),
        h(
          "div",
          { className: "el-bar" },
          chips,
          h("span", { className: "gl-spacer" }),
          h("label", { className: "el-sound", title: "Record or import the session first and break it into words (Auto-segment by word); then each answer you type is heard in the segment selected on the Timeline, and the next segment plays" }, soundBox, " Sound from the Timeline"),
        ),
        soundHint,
        h("ul", { className: "el-rows" }, ...rows),
        h("p", { className: "note lx-attr", textContent: `${s.citation} · ${CONCEPTICON}` }),
      ];
    }

    // ------------------------------------------------------------------ domains

    function domainsBody(): HTMLElement[] {
      const all = domains ?? [];
      if (all.length === 0) {
        return [
          h(
            "div",
            { className: "lx-picker" },
            h("p", { textContent: "Collect words by meaning, one semantic domain at a time, with the SIL elicitation questions." }),
            h("button", {
              type: "button",
              textContent: "Load SIL Semantic Domains",
              onclick: () =>
                void enqueue(async () => {
                  const r = await domainsLoadSil();
                  notes(r.notes);
                  domains = await domainsList();
                  touchLexicon();
                  draw();
                }),
            }),
            h("p", { className: "note lx-attr", textContent: SIL }),
          ),
        ];
      }
      const search = h("input", { type: "search", value: domainQuery, placeholder: "Find a domain: a word or a number (e.g. 1.1.1)", ariaLabel: "Find a semantic domain" });
      const results = h("ul", { className: "lx-options el-domains" });
      const shown = () => filterDomains(emptyOnly ? all.filter((d) => (d.words ?? 0) === 0) : all, domainQuery, 60);
      const open = (id: string) =>
        void enqueue(async () => {
          domain = await domainQuestions({ domainId: id });
          draw();
          el.querySelector<HTMLInputElement>(".el-collect input")?.focus();
        });
      const emptyBox = h("input", { type: "checkbox", checked: emptyOnly });
      emptyBox.onchange = () => {
        emptyOnly = emptyBox.checked;
        paint();
      };
      const next = nextEmptyDomain(all, domain?.id ?? null);
      const coverage = h(
        "div",
        { className: "el-coverage" },
        h("p", { className: "note", textContent: coverageLine(all) }),
        h("label", { className: "el-empty-only" }, emptyBox, " Only domains with no words yet"),
        next === null
          ? null
          : h("button", { type: "button", textContent: "Next empty domain", title: `Go on to ${next.code} ${next.label}, the next domain in the outline with no words yet, as a Rapid Word Collection session does`, onclick: () => open(next.id) }),
      );
      const paint = () => {
        results.replaceChildren(
          ...shown().map((d) =>
            h(
              "li",
              {
                className: domain?.id === d.id ? "on" : "",
                title: d.description,
                onclick: () => open(d.id),
              },
              h("span", { className: "lx-code", textContent: d.code }),
              ` ${d.label}`,
              (d.words ?? 0) > 0 ? h("span", { className: "el-count", textContent: String(d.words), title: `${d.words} ${d.words === 1 ? "word" : "words"} collected here` }) : null,
            ),
          ),
        );
        const list = shown();
        for (const [i, li] of [...results.children].entries()) {
          const d = list[i];
          if (d !== undefined && li instanceof HTMLElement) li.style.paddingLeft = `${6 + Math.min(d.depth, 6) * 10}px`;
        }
      };
      search.oninput = () => {
        domainQuery = search.value;
        paint();
      };
      paint();
      const left = h("div", { className: "el-domain-list" }, coverage, search, results);
      return [h("div", { className: "lx-body" }, left, h("div", { className: "lx-entry" }, domain === null ? h("p", { className: "note", textContent: "Pick a domain to see its questions." }) : domainDetail(domain)))];
    }

    function domainDetail(d: DomainQuestions): HTMLElement {
      const form = typeWithShortcuts(h("input", { placeholder: "Word…", ariaLabel: `Word for ${d.label}` }));
      const gloss = h("input", { placeholder: "Gloss", ariaLabel: "Gloss", className: "el-gloss" });
      const save = () => {
        const typed = form.value.trim();
        if (typed === "") return;
        const g = gloss.value;
        form.value = "";
        gloss.value = "";
        void enqueue(async () => {
          const r = await domainCollect({ domainId: d.id, form: typed, gloss: g });
          domain = r.domain;
          domains = await domainsList();
          notes(r.notes);
          touchLexicon();
          draw();
          el.querySelector<HTMLInputElement>(".el-collect input")?.focus();
        });
      };
      for (const input of [form, gloss]) {
        input.onkeydown = (ev) => {
          if (!ev.isComposing && ev.key === "Enter") {
            ev.preventDefault();
            save();
          }
        };
      }
      return h(
        "section",
        {},
        h("h1", {}, h("span", { className: "lx-code", textContent: `${d.code} ` }), d.label),
        d.description === "" ? null : h("p", { className: "note", textContent: d.description }),
        h("div", { className: "el-collect form inline" }, form, gloss, h("button", { type: "button", textContent: "Add", onclick: save })),
        h("div", { className: "lx-domains" }, ...d.words.map((w) => h("span", { className: "lx-chip", title: w.gloss }, h("strong", { textContent: headword(w.lemma, w.homograph) }), w.gloss === "" ? "" : ` ‘${w.gloss}’`))),
        d.questions.length === 0
          ? null
          : h(
              "ol",
              { className: "el-questions" },
              ...d.questions.map((q) => h("li", {}, h("span", { textContent: q.q }), q.words === "" ? null : h("span", { className: "note", textContent: ` e.g. ${q.words}` }))),
            ),
        h("p", { className: "note lx-attr", textContent: SIL }),
      );
    }

    // ------------------------------------------------------------------ frame

    function draw() {
      const tabs = h(
        "div",
        { className: "scopes" },
        ...(["lists", "domains"] as const).map((m) =>
          h("button", {
            type: "button",
            textContent: m === "lists" ? "Word lists" : "Semantic domains",
            ariaPressed: String(mode === m),
            onclick: () => {
              mode = m;
              void enqueue(async () => {
                if (m === "domains" && domains === null) domains = await domainsList();
                draw();
              });
            },
          }),
        ),
      );
      el.replaceChildren(tabs, ...(mode === "lists" ? listsBody() : domainsBody()));
      drawOffers();
    }

    const view_: PanelView = {
      element: el,
      dispose: () => unwatchSelection(),
      render() {
        const s = getState();
        const key = s.kind === "open" ? `${s.info.path}|${dataGeneration()}|${lexiconRevision()}` : "closed";
        if (key === shownFor) return;
        const sameProject = s.kind === "open" && shownFor.split("|")[0] === s.info.path;
        // Mid-answer: the panel already shows its own saves; catch up once
        // the user leaves the field.
        if (sameProject && document.activeElement instanceof HTMLInputElement && el.contains(document.activeElement)) {
          if (!refreshWaiting) {
            refreshWaiting = true;
            el.addEventListener("focusout", () => window.setTimeout(() => {
              refreshWaiting = false;
              // Focus moved to the next answer field: wait for that one too.
              view_.render();
            }), { once: true });
          }
          return;
        }
        shownFor = key;
        if (s.kind !== "open") {
          el.replaceChildren(emptyState("Elicit", "Open a project to work through a word list or the semantic domains with a speaker."));
          return;
        }
        if (!sameProject) {
          projectGen += 1;
          drafts.clear();
          pending.clear();
          moveTo(null);
          view = null;
          domains = null;
          domain = null;
          current = null;
          listId = localStorage.getItem(LIST_KEY);
        }
        void enqueue(async () => {
          await loadLists();
          if (mode === "domains") domains = await domainsList();
          if (domain !== null) domain = await domainQuestions({ domainId: domain.id }).catch(() => null);
          draw();
        });
      },
    };
    return view_;
  },
};
