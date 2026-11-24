// Elicit sentences: a list of prompts (a questionnaire's sentences in a
// contact language) worked through one by one. Each answer is saved as a
// sentence of the list's own text with the prompt as its translation, so it
// is glossed, searched and exported like any other; several answers to one
// prompt are kept. Commands in src-tauri/src/sentences.rs; rules in
// ../sentences/logic.ts.
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { elicitationSheet } from "../elicit/sheet";
import { errorText, lexiconList, projectLanguage, saveDocument, sentenceAnswer, sentenceListImport, sentenceListMake, sentenceLists, sentenceListView, type EntryRow } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { framedLine, framePrompts, NO_LISTS, nextToDo, progressLine, shownPrompts, type PromptList, type PromptRow, type PromptView } from "../sentences/logic";
import { dataGeneration, getState, selectSegment, selectText, showToast } from "../store";
import { typeWithShortcuts } from "../typing";
import { SHOW_PANEL, type PanelDef, type PanelView } from "./panel";

export const sentencesPanel: PanelDef = {
  title: "Elicit sentences",
  create(): PanelView {
    const el = h("div", { className: "panel panel-sentences" });
    let project = "";
    let generation = -1;
    let lists: PromptList[] = [];
    let listId = "";
    let view: PromptView | null = null;
    let toDoOnly = false;
    let seq = 0;
    /** Typed but not saved, by prompt id, kept across redraws. */
    const drafts = new Map<string, string>();
    /** The prompt whose field should have focus after the next draw. */
    let focusOn: string | null = null;

    const picker = h("select", { className: "se-list" });
    picker.setAttribute("aria-label", "Prompt list");
    const load = h("button", { type: "button", textContent: "Load prompts…", title: "Read a list of prompts: a .txt with one sentence a line, or a CSV or tab-separated table with a prompt column" });
    const toDoBox = h("input", { type: "checkbox" });
    const toDo = h("label", { title: "List only the prompts that have no answer yet" }, toDoBox, " Only prompts to do");
    const print = h("button", { type: "button", textContent: "Print a sheet…", title: "These prompts as a form to print and fill in by hand: a header for the speaker, date and place, then a row per prompt with room for the sentence and notes. With Only prompts to do ticked, only those" });
    const line = h("p", { className: "note se-line" });
    // A list made from frames: each lexicon entry's gloss put in sentence frames.
    const framesButton = h("button", { type: "button", textContent: "From frames…", title: "Make a list from frames you write, such as «I see the ___»: each lexicon entry's gloss in each frame, the headword beside it, as tone and noun-class frames are asked" });
    const frameName = h("input", { type: "text", value: "Frames", ariaLabel: "Name of the list" });
    const frameText = h("textarea", { rows: 4, className: "se-frame-text", placeholder: "I see the ___.\nThe ___ is big.\n___", ariaLabel: "Frames, one a line, ___ where the word goes" });
    const frameCategory = h("select", { ariaLabel: "Which entries" });
    const frameNote = h("p", { className: "note" });
    const frameMake = h("button", { type: "button", textContent: "Make the list" });
    const frameBox = h(
      "div",
      { className: "se-frames", hidden: true },
      h("label", {}, "Name ", frameName),
      h("label", {}, "Entries ", frameCategory),
      frameText,
      frameNote,
      h("div", { className: "cc-bar" }, frameMake),
    );
    let frameEntries: EntryRow[] = [];
    const framed = () => framePrompts(frameText.value, frameEntries, frameCategory.value === "" ? null : frameCategory.value);
    const drawFramed = () => {
      const f = framed();
      frameNote.textContent = frameText.value.trim() === "" ? "Write one frame a line, with ___ where the word goes; ___ alone asks for the word on its own." : framedLine(f);
      frameMake.disabled = f.prompts.length === 0;
    };
    const list = h("ol", { className: "se-prompts" });

    const fetchView = async () => {
      const mine = ++seq;
      if (listId === "") {
        view = null;
        draw();
        return;
      }
      try {
        const v = await sentenceListView({ list: listId });
        if (mine !== seq) return;
        view = v;
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      draw();
    };

    const fetchLists = async () => {
      try {
        lists = await sentenceLists();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
        return;
      }
      if (!lists.some((l) => l.id === listId)) listId = lists[0]?.id ?? "";
      picker.replaceChildren(...lists.map((l) => h("option", { value: l.id, textContent: `${l.name} (${l.answered} of ${l.total})`, selected: l.id === listId })));
      await fetchView();
    };

    const save = async (r: PromptRow, field: HTMLInputElement) => {
      const sentence = field.value.trim();
      if (sentence === "") return;
      field.disabled = true;
      try {
        view = await sentenceAnswer({ list: listId, prompt: r.id, sentence });
        drafts.delete(r.id);
        focusOn = nextToDo(view.prompts, r.id)?.id ?? null;
        const l = lists.find((x) => x.id === listId);
        if (l !== undefined) Object.assign(l, view.list);
        picker.querySelector<HTMLOptionElement>(`option[value="${CSS.escape(listId)}"]`)?.replaceChildren(`${view.list.name} (${view.list.answered} of ${view.list.total})`);
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
        focusOn = r.id;
      }
      draw();
    };

    const openAnswer = async (textId: string, a: { segmentId: string; tierId: string }) => {
      await selectText(textId);
      selectSegment({ textId, tierId: a.tierId, segmentId: a.segmentId });
      window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "gloss" }));
    };

    const draw = () => {
      if (lists.length === 0) {
        line.textContent = NO_LISTS;
        list.replaceChildren();
        return;
      }
      if (view === null) {
        line.textContent = "Loading…";
        return;
      }
      const v = view;
      line.textContent = progressLine(v.list);
      list.replaceChildren(
        ...shownPrompts(v.prompts, toDoOnly).map((r) => {
          const field = typeWithShortcuts(h("input", { type: "text", className: "se-answer", placeholder: r.answers.length === 0 ? (r.hint ? `Say it with «${r.hint}»…` : "Say it in the language…") : "Another way to say it…", value: drafts.get(r.id) ?? "" }));
          field.setAttribute("aria-label", `Answer to prompt ${r.number}: ${r.prompt}`);
          field.dataset.prompt = r.id;
          field.oninput = () => drafts.set(r.id, field.value);
          field.onkeydown = (e) => {
            if (e.key === "Enter" && !e.isComposing) {
              e.preventDefault();
              void save(r, field);
            }
          };
          const chips = r.answers.map((a) => {
            const b = h("button", { type: "button", className: "se-chip", textContent: a.content, title: "Open this sentence in the Gloss panel" });
            b.onclick = () => v.list.textId !== null && void openAnswer(v.list.textId, a);
            return b;
          });
          return h(
            "li",
            { className: `se-row${r.answers.length > 0 ? " se-done" : ""}` },
            h("div", { className: "se-prompt" }, h("span", { className: "se-num", textContent: `${r.number}.` }), h("span", { textContent: r.prompt }), r.hint ? h("span", { className: "se-hint", textContent: r.hint, title: "The word this frame asks for" }) : null),
            chips.length === 0 ? null : h("div", { className: "se-chips" }, ...chips),
            field,
          );
        }),
      );
      if (focusOn !== null) {
        const f = list.querySelector<HTMLInputElement>(`input[data-prompt="${CSS.escape(focusOn)}"]`);
        focusOn = null;
        f?.focus();
        f?.scrollIntoView({ block: "nearest" });
      }
    };

    print.onclick = async () => {
      const v = view;
      if (v === null) return;
      const folder = await pickFile({ directory: true, title: "Where should the sheet go?" });
      if (typeof folder !== "string") return;
      try {
        const language = await projectLanguage().catch(() => ({ name: "", glottocode: "", iso: "" }));
        const contents = elicitationSheet({
          list: { name: v.list.name, citation: "" },
          language: language.name,
          concepts: v.prompts.map((r) => ({ number: r.number, gloss: r.hint ? `${r.prompt} [${r.hint}]` : r.prompt, concepticonGloss: "", skipped: false, answers: r.answers.map((a) => ({ lemma: a.content, gloss: "" })) })),
          which: toDoOnly ? "todo" : "all",
          withAnswers: !toDoOnly,
          generator: "wugs",
          kind: "sentences",
        });
        const r = await saveDocument({ folder, title: `${v.list.name} sheet`, extension: "html", contents });
        showToast({ tone: "info", lines: [`Saved a sheet to print in ${r.path}. Open it in a browser or Word and print it.`] });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
    };
    framesButton.onclick = async () => {
      frameBox.hidden = !frameBox.hidden;
      if (frameBox.hidden) return;
      try {
        frameEntries = await lexiconList({ query: "" });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
        return;
      }
      const counts = new Map<string, number>();
      for (const e of frameEntries) if (e.posOfSpeech !== "") counts.set(e.posOfSpeech, (counts.get(e.posOfSpeech) ?? 0) + 1);
      const was = frameCategory.value;
      frameCategory.replaceChildren(
        h("option", { value: "", textContent: `Every entry (${frameEntries.length})` }),
        ...[...counts].sort((a, b) => b[1] - a[1]).map(([c, n]) => h("option", { value: c, textContent: `${c} (${n})`, selected: c === was })),
      );
      drawFramed();
      frameText.focus();
    };
    frameText.oninput = drawFramed;
    frameCategory.onchange = drawFramed;
    frameMake.onclick = async () => {
      const f = framed();
      if (f.prompts.length === 0) return;
      try {
        const l = await sentenceListMake({ name: frameName.value, prompts: f.prompts });
        listId = l.id;
        frameBox.hidden = true;
        showToast({ tone: "info", lines: [`${l.name}: ${l.total} prompts, ${l.answered} answered already.`] });
        await fetchLists();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
    };
    picker.onchange = () => {
      listId = picker.value;
      drafts.clear();
      void fetchView();
    };
    toDoBox.onchange = () => {
      toDoOnly = toDoBox.checked;
      draw();
    };
    load.onclick = async () => {
      const picked = await pickFile({ multiple: false, title: "A list of prompts", filters: [{ name: "Prompts", extensions: ["txt", "csv", "tsv", "tab", "xlsx", "xls", "ods"] }] });
      if (typeof picked !== "string") return;
      try {
        const l = await sentenceListImport({ path: picked });
        listId = l.id;
        showToast({ tone: "info", lines: [`${l.name}: ${l.total} prompts, ${l.answered} answered already.`] });
        await fetchLists();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
    };

    const unprovide = provide("sentences", () => [
      { id: "sentences:load", title: "Load prompts for sentence elicitation", group: "Elicit sentences", words: ["questionnaire", "stimulus", "translation", "elicitation", "prompt"], run: () => load.click() },
      {
        id: "sentences:next",
        title: "Go to the next prompt without an answer",
        group: "Elicit sentences",
        run: () => {
          if (view === null) return;
          const active = (document.activeElement as HTMLElement | null)?.dataset?.prompt ?? null;
          focusOn = nextToDo(view.prompts, active)?.id ?? null;
          draw();
        },
      },
    ]);

    const render = () => {
      const s = getState();
      if (s.kind !== "open") {
        project = "";
        el.replaceChildren(emptyState("Elicit sentences", "Open a project to elicit sentences."));
        return;
      }
      if (s.info.projectId !== project) {
        project = s.info.projectId;
        generation = dataGeneration();
        listId = "";
        view = null;
        drafts.clear();
        el.replaceChildren(h("div", { className: "cc-bar" }, picker, load, framesButton, toDo, print), frameBox, line, list);
        void fetchLists();
        return;
      }
      // Another panel or device changed the data: read the list again, unless a field is being typed in.
      if (dataGeneration() !== generation && !el.contains(document.activeElement)) {
        generation = dataGeneration();
        void fetchLists();
      }
    };
    return { element: el, render, dispose: unprovide };
  },
};
