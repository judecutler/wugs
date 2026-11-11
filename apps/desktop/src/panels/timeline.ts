// The canvas timeline (TONGUES-UI build order step 2): waveform + tier
// lanes + segment editing. The canvas work lives in ../timeline/view.ts;
// this file is the panel chrome around it: source picker, import, the
// tier gutter, the editing buttons (Auto-segment, Split, Merge, Undo, Redo)
// and the perf toggle.
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { addTier, importMedia, listSources, peaksInfo, type SourceRow, type TierTemplate } from "../api";
import { emptyState, h } from "../dom";
import { praatControls } from "../praat/control";
import { attempt, dataGeneration, getSelection, getState, refreshText } from "../store";
import { RULER_H, TimelineView, WAVE_H, TIER_H, timelineHistory } from "../timeline/view";
import type { PanelDef, PanelView } from "./panel";

const TEMPLATES: TierTemplate[] = ["transcription", "translation", "notes", "custom"];

export const timelinePanel: PanelDef = {
  title: "Timeline",
  create(): PanelView {
    const el = h("div", { className: "panel panel-timeline" });
    let view: TimelineView | null = null;
    let sources: SourceRow[] = [];
    let selectedSource: string | null = null;
    let lastSlice = "";
    let lastText = "";
    let lastGeneration = -1;
    let lastProject = "";

    const sourcePicker = h("select", { className: "tl-sources", title: "The recording behind this timeline" });
    sourcePicker.onchange = () => void loadSource(sourcePicker.value || null);
    const importButton = h("button", { textContent: "Import audio…", title: "Add a WAV recording to this project" });
    const perfButton = h("button", { textContent: "Perf", title: "Show frame times and strip-cache hits on the canvas" });
    const praat = praatControls(() => {
      const s = getState();
      if (s.kind !== "open" || s.text === null || view === null) return null;
      const sel = getSelection();
      const seg = sel !== null && sel.textId === s.text.id ? s.text.segments.find((x) => x.id === sel.segmentId) ?? null : null;
      return { textId: s.text.id, sourceId: selectedSource, selected: seg, visible: view.visibleRange() };
    }, () => view?.analysis ?? null);
    const autoButton = h("button", {
      textContent: "Auto-segment",
      title: "Add a segment for each stretch of speech in this recording (on the selected segment's track, else the utterance track). Segments you drew are left alone; Undo takes them all back.",
    });
    const splitButton = h("button", { textContent: "Split", title: "Cut the selected segment at the playhead (S). Its text stays on the first part." });
    const mergeButton = h("button", { textContent: "Merge with next", title: "Join the selected segment with the next one on its track (M)." });
    const undoButton = h("button", { textContent: "Undo", title: "Undo (Ctrl+Z)" });
    const redoButton = h("button", { textContent: "Redo", title: "Redo (Ctrl+Shift+Z or Ctrl+Y)" });
    const editTools = h("span", { className: "tl-edit-tools", title: "Drawn and dragged edges snap to where speech starts and ends; hold Alt to place them freely. [ ] nudge the start, Shift+[ ] the end." }, autoButton, splitButton, mergeButton, undoButton, redoButton);
    const toolbar = h("div", { className: "tl-toolbar" }, sourcePicker, importButton, editTools, perfButton, praat.element);

    const updateEditButtons = () => {
      const stack = view?.history() ?? null;
      undoButton.disabled = stack === null || !stack.canUndo();
      redoButton.disabled = stack === null || !stack.canRedo();
      undoButton.title = stack?.undoLabel() != null ? `Undo ${stack.undoLabel()} (Ctrl+Z)` : "Undo (Ctrl+Z)";
      redoButton.title = stack?.redoLabel() != null ? `Redo ${stack.redoLabel()} (Ctrl+Shift+Z or Ctrl+Y)` : "Redo (Ctrl+Shift+Z or Ctrl+Y)";
      autoButton.disabled = autoButton.dataset.busy === "1" || view === null || selectedSource === null;
    };
    timelineHistory.onChange = updateEditButtons;
    undoButton.onclick = () => void view?.undo();
    redoButton.onclick = () => void view?.redo();
    splitButton.onclick = () => void view?.splitAtPlayhead();
    mergeButton.onclick = () => void view?.mergeWithNext();
    autoButton.onclick = () => {
      const source = selectedSource;
      if (view === null || source === null) return;
      autoButton.dataset.busy = "1";
      autoButton.textContent = "Finding speech…";
      updateEditButtons();
      void view.autoSegment(source).finally(() => {
        autoButton.dataset.busy = "";
        autoButton.textContent = "Auto-segment";
        updateEditButtons();
      });
    };
    const gutter = h("div", { className: "tl-gutter" });
    const body = h("div", { className: "tl-body" });
    const main = h("div", { className: "tl-main" }, gutter, body);

    perfButton.onclick = () => {
      if (view !== null) {
        view.showPerf = !view.showPerf;
        perfButton.classList.toggle("active", view.showPerf);
      }
    };

    importButton.onclick = () => {
      void attempt(async () => {
        const path = await pickFile({ title: "Import a WAV recording", filters: [{ name: "WAV audio", extensions: ["wav"] }] });
        if (typeof path !== "string") return;
        importButton.disabled = true;
        try {
          const row = await importMedia({ path });
          await refreshSources();
          await loadSource(row.id);
        } finally {
          importButton.disabled = false;
        }
      });
    };

    async function refreshSources(): Promise<void> {
      sources = (await listSources()).filter((s) => s.kind === "audio" && s.hasMedia);
      sourcePicker.replaceChildren(
        h("option", { value: "", textContent: sources.length === 0 ? "No recordings yet" : "Pick a recording…" }),
        ...sources.map((s) => h("option", { value: s.id, textContent: s.label || s.id })),
      );
      if (selectedSource !== null && sources.some((s) => s.id === selectedSource)) {
        sourcePicker.value = selectedSource;
      } else if (selectedSource !== null) {
        selectedSource = null;
        view?.setMedia(null, null);
      }
      // One recording and nothing picked: load it without a click.
      const only = sources.length === 1 ? sources[0] : undefined;
      if (selectedSource === null && only !== undefined) await loadSource(only.id);
    }

    async function loadSource(sourceId: string | null): Promise<void> {
      selectedSource = sourceId;
      if (view === null) return;
      if (sourceId === null) {
        view.setMedia(null, null);
        return;
      }
      await attempt(async () => {
        view?.setMedia(await peaksInfo({ sourceId }), sourceId);
      });
      sourcePicker.value = sourceId;
      praat.refresh();
      updateEditButtons();
    }

    function addTrackControl(): HTMLElement {
      const holder = h("div", { className: "tl-add-track" });
      const menu = h(
        "select",
        { title: "Add a track (a tier) to this project" },
        h("option", { value: "", textContent: "Add track…" }),
        ...TEMPLATES.map((t) => h("option", { value: t, textContent: t })),
      );
      const create = (template: TierTemplate, name: string | null) =>
        void attempt(async () => {
          await addTier({ template, name });
          await refreshText();
        });
      menu.onchange = () => {
        const template = menu.value as TierTemplate | "";
        menu.value = "";
        if (template === "") return;
        if (template !== "custom") {
          create(template, null);
          return;
        }
        // A custom track needs a name; script dialogs are unreliable
        // across webviews, so ask inline.
        const nameInput = h("input", { placeholder: "Track name" });
        nameInput.onkeydown = (ev) => {
          if (ev.key === "Enter" && nameInput.value.trim() !== "") create("custom", nameInput.value.trim());
          else if (ev.key === "Escape") holder.replaceChildren(menu);
        };
        holder.replaceChildren(nameInput);
        nameInput.focus();
      };
      holder.replaceChildren(menu);
      return holder;
    }

    function placed(className: string, top: number, height: number | null, ...children: (Node | string)[]): HTMLDivElement {
      const div = h("div", { className }, ...children);
      div.style.top = `${top}px`;
      if (height !== null) div.style.height = `${height}px`;
      return div;
    }

    function drawGutter(): void {
      if (view === null) return;
      const names = view.laneNames();
      gutter.replaceChildren(
        placed("tl-gutter-wave", RULER_H, WAVE_H, "waveform"),
        ...names.map((name, i) => placed("tl-gutter-tier", RULER_H + WAVE_H + i * TIER_H, TIER_H, name)),
        placed("tl-gutter-add", RULER_H + WAVE_H + names.length * TIER_H + 6, null, addTrackControl()),
      );
    }

    const render = () => {
      const s = getState();
      const slice = s.kind === "open" ? `${s.info.projectId}|${s.text?.id ?? ""}` : "closed";
      if (s.kind === "closed") {
        if (slice !== lastSlice) {
          view?.dispose();
          view = null;
          selectedSource = null;
          el.replaceChildren(
            emptyState(
              "No project open",
              "Enter your researcher id, then open a project folder or create a new one. A second device can join from the Devices panel.",
              "The wug is waiting for its first recording.",
            ),
          );
        }
        lastSlice = slice;
        return;
      }
      if (s.text === null) {
        if (slice !== lastSlice) {
          view?.dispose();
          view = null;
          el.replaceChildren(
            emptyState(
              "Timeline",
              s.texts.length === 0 ? "Create a text to start transcribing." : "Select a text to see it on the timeline.",
              "The wug flew over an empty valley.",
            ),
          );
        }
        lastSlice = slice;
        return;
      }
      if (view === null) {
        view = new TimelineView();
        view.analysis.onChange = () => praat.refresh();
        body.replaceChildren(view.element);
        el.replaceChildren(toolbar, praat.review, main);
        lastText = "";
        lastProject = "";
      }
      if (lastProject !== s.info.projectId) {
        // Another project's recordings are meaningless here.
        lastProject = s.info.projectId;
        selectedSource = null;
        view.setMedia(null, null);
        lastGeneration = dataGeneration();
        void refreshSources();
      } else if (lastGeneration !== dataGeneration()) {
        lastGeneration = dataGeneration();
        void refreshSources();
      }
      const textStamp = `${s.text.id}|${JSON.stringify(s.text.tiers)}|${JSON.stringify(s.text.segments)}`;
      if (textStamp !== lastText) {
        lastText = textStamp;
        view.setText(s.text);
        drawGutter();
      }
      lastSlice = slice;
      praat.refresh();
      updateEditButtons();
    };

    return {
      element: el,
      render,
      dispose: () => {
        view?.dispose();
        view = null;
      },
    };
  },
};
