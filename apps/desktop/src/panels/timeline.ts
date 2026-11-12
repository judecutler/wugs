// The canvas timeline (TONGUES-UI build order step 2): waveform + tier
// lanes + segment editing. The canvas work lives in ../timeline/view.ts;
// this file is the panel chrome around it: source picker, import, the
// tier gutter, the editing buttons (Auto-segment, Split, Merge, Undo, Redo)
// and the perf toggle.
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { addTier, importMedia, listSources, peaksInfo, type SourceRow, type TierTemplate } from "../api";
import { emptyState, h } from "../dom";
import { praatControls } from "../praat/control";
import { attempt, dataGeneration, getSelection, getState, refreshText, takeWantedMedia } from "../store";
import { parseGranularity } from "../timeline/edit-rules";
import { RULER_H, TimelineView, WAVE_H, TIER_H, timelineHistory } from "../timeline/view";
import { clock } from "../timeline/tracks";
import { ICONS, type IconName } from "../timeline/icons";
import * as playback from "../playback";
import type { PanelDef, PanelView } from "./panel";

const TEMPLATES: TierTemplate[] = ["transcription", "translation", "notes", "custom"];
/** Auto-segment's last granularity on this machine. */
const GRANULARITY_KEY = "wugs.timeline.autoSegmentBy";

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
    const importButton = h("button", { textContent: "Import audio…", title: "Add a recording to this project (WAV, M4A, MP3, FLAC, Ogg, AIFF; compressed files are decoded to WAV)" });
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
    const byPicker = h(
      "select",
      { className: "tl-auto-by", title: "Auto-segment makes one segment per utterance (split at real pauses) or per word (split at short pauses too)" },
      h("option", { value: "utterance", textContent: "by utterance" }),
      h("option", { value: "word", textContent: "by word" }),
    );
    byPicker.value = parseGranularity(localStorage.getItem(GRANULARITY_KEY));
    byPicker.onchange = () => localStorage.setItem(GRANULARITY_KEY, byPicker.value);
    const SPLIT_TITLE = "Cut the selected segment at the playhead (S). Its text stays on the first part.";
    const MERGE_TITLE = "Join the selected segment with the next one on its track (M).";
    const splitButton = h("button", { textContent: "Split", title: SPLIT_TITLE });
    const mergeButton = h("button", { textContent: "Merge with next", title: MERGE_TITLE });
    const undoButton = h("button", { textContent: "Undo", title: "Undo (Ctrl+Z)" });
    const redoButton = h("button", { textContent: "Redo", title: "Redo (Ctrl+Shift+Z or Ctrl+Y)" });
    const editTools = h("span", { className: "tl-edit-tools", title: "Drawn and dragged edges snap to where speech starts and ends; hold Alt to place them freely. [ ] nudge the start, Shift+[ ] the end." }, autoButton, byPicker, splitButton, mergeButton, undoButton, redoButton);
    const toolbar = h("div", { className: "tl-toolbar" }, sourcePicker, importButton, editTools, perfButton, praat.element);

    const updateEditButtons = () => {
      const stack = view?.history() ?? null;
      undoButton.disabled = stack === null || !stack.canUndo();
      redoButton.disabled = stack === null || !stack.canRedo();
      undoButton.title = stack?.undoLabel() != null ? `Undo ${stack.undoLabel()} (Ctrl+Z)` : "Undo (Ctrl+Z)";
      redoButton.title = stack?.redoLabel() != null ? `Redo ${stack.redoLabel()} (Ctrl+Shift+Z or Ctrl+Y)` : "Redo (Ctrl+Shift+Z or Ctrl+Y)";
      autoButton.disabled = autoButton.dataset.busy === "1" || view === null || selectedSource === null;
      byPicker.disabled = autoButton.disabled;
      // The same rule as the S and M keys; the title says why when disabled.
      const rules = view?.editRules() ?? { split: "Open a text first.", merge: "Open a text first.", next: null };
      splitButton.disabled = rules.split !== null;
      splitButton.title = rules.split === null ? SPLIT_TITLE : `${SPLIT_TITLE} ${rules.split}`;
      mergeButton.disabled = rules.merge !== null;
      mergeButton.title = rules.merge === null ? MERGE_TITLE : `${MERGE_TITLE} ${rules.merge}`;
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
      void view.autoSegment(source, parseGranularity(byPicker.value)).finally(() => {
        autoButton.dataset.busy = "";
        autoButton.textContent = "Auto-segment";
        updateEditButtons();
      });
    };
    // Transport (docs/design/EDITOR.md): play, the clock, and zoom, laid
    // out the way video editors put them along the top of a timeline.
    const playButton = iconButton("play", "Play (Space)");
    const startButton = iconButton("start", "Go to start (Home)");
    const endButton = iconButton("end", "Go to end (End)");
    const clockNow = h("span", { className: "tl-clock-now", textContent: "0:00.0" });
    const clockTotal = h("span", { className: "tl-clock-total", textContent: "0:00.0" });
    const clockBox = h("span", { className: "tl-clock", title: "Playhead and length. J steps back, K stops, L plays." }, clockNow, " / ", clockTotal);
    const zoomOut = h("button", { className: "tl-zoom-step", textContent: "−", title: "Zoom out (-)" });
    const zoomSlider = h("input", { className: "tl-zoom", type: "range", min: "0", max: "1000", step: "1", value: "0", title: "Zoom" });
    zoomSlider.setAttribute("aria-label", "Zoom");
    const zoomIn = h("button", { className: "tl-zoom-step", textContent: "+", title: "Zoom in (+)" });
    const fitButton = h("button", { textContent: "Fit", title: "Show the whole recording (Shift+Z)" });
    const transport = h(
      "div",
      { className: "tl-transport" },
      h("span", { className: "tl-transport-play" }, startButton, playButton, endButton, clockBox),
      h("span", { className: "tl-transport-zoom" }, zoomOut, zoomSlider, zoomIn, fitButton),
    );
    playButton.onclick = () => view?.playPause();
    startButton.onclick = () => view?.seekTo(0);
    endButton.onclick = () => view?.seekTo(view.durationMs());
    zoomOut.onclick = () => view?.zoomBy(1 / 1.5);
    zoomIn.onclick = () => view?.zoomBy(1.5);
    fitButton.onclick = () => view?.fit();
    zoomSlider.oninput = () => view?.setZoomSlider(Number(zoomSlider.value));

    // The clock, play icon and slider follow the view every frame, so a
    // wheel zoom or another panel's playback shows here too.
    let shown = { playing: false, now: "", total: "", zoom: -1, hasRecording: true };
    let tick = 0;
    function follow(): void {
      tick = requestAnimationFrame(follow);
      if (view === null) return;
      const hasRecording = view.hasRecording();
      const playing = hasRecording && playback.isPlaying();
      const now = clock(hasRecording ? playback.currentMs() : 0);
      const total = clock(view.durationMs());
      const zoom = view.zoomSlider();
      if (playing !== shown.playing) {
        playButton.innerHTML = ICONS[playing ? "pause" : "play"];
        playButton.title = playing ? "Pause (Space)" : "Play (Space)";
        playButton.setAttribute("aria-label", playing ? "Pause" : "Play");
      }
      if (hasRecording !== shown.hasRecording) {
        for (const b of [playButton, startButton, endButton]) b.disabled = !hasRecording;
      }
      if (now !== shown.now) clockNow.textContent = now;
      if (total !== shown.total) clockTotal.textContent = total;
      if (zoom !== shown.zoom && document.activeElement !== zoomSlider) zoomSlider.value = String(zoom);
      shown = { playing, now, total, zoom, hasRecording };
    }

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
        const path = await pickFile({
          title: "Import a recording",
          filters: [
            { name: "Audio (WAV, M4A, MP3, FLAC, Ogg, AIFF)", extensions: ["wav", "m4a", "mp4", "aac", "mp3", "flac", "ogg", "oga", "aif", "aiff"] },
            { name: "All files", extensions: ["*"] },
          ],
        });
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
      const wanted = takeWantedMedia(sources.map((s) => s.id));
      if (wanted !== null) {
        await loadSource(wanted);
        return;
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

    function placed(className: string, top: number, height: number | null, ...children: (Node | string | null)[]): HTMLDivElement {
      const div = h("div", { className }, ...children);
      div.style.top = `${top}px`;
      if (height !== null) div.style.height = `${height}px`;
      return div;
    }

    /** Track headers: a name and its toggles, like a video editor's track header. */
    function drawGutter(): void {
      if (view === null) return;
      const v = view;
      const rows = v.trackRows();
      const muteButton = toggleButton(v.muted, "muted", "sound", v.muted ? "Unmute the recording" : "Mute the recording", () => v.toggleMute());
      gutter.replaceChildren(
        placed("tl-gutter-wave", RULER_H, WAVE_H, h("span", { className: "tl-track-name", textContent: "Recording" }), h("span", { className: "tl-track-toggles" }, muteButton)),
        ...rows.map((row, i) => {
          const tierId = row.tierId;
          const toggles =
            tierId === null
              ? null
              : h(
                  "span",
                  { className: "tl-track-toggles" },
                  toggleButton(row.locked, "lock", "unlock", row.locked ? `Unlock ${row.name}` : `Lock ${row.name} so its segments cannot change`, () => v.toggleTrack(tierId, "locked")),
                  toggleButton(row.hidden, "eyeOff", "eye", row.hidden ? `Show ${row.name}` : `Hide ${row.name}`, () => v.toggleTrack(tierId, "hidden")),
                );
          const div = placed("tl-gutter-tier", RULER_H + WAVE_H + i * TIER_H, TIER_H, h("span", { className: "tl-track-name", textContent: row.name, title: row.name }), toggles);
          div.classList.toggle("is-hidden", row.hidden);
          div.classList.toggle("is-locked", row.locked);
          return div;
        }),
        placed("tl-gutter-add", RULER_H + WAVE_H + rows.length * TIER_H + 6, null, addTrackControl()),
      );
    }

    const render = () => {
      const s = getState();
      const slice = s.kind === "open" ? `${s.info.projectId}|${s.text?.id ?? ""}` : "closed";
      if (s.kind === "closed") {
        if (slice !== lastSlice) {
          view?.dispose();
          view = null;
          cancelAnimationFrame(tick);
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
          cancelAnimationFrame(tick);
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
        view.onEditStateChange = updateEditButtons;
        view.onTracksChange = drawGutter;
        body.replaceChildren(view.element);
        el.replaceChildren(toolbar, transport, praat.review, main);
        cancelAnimationFrame(tick);
        tick = requestAnimationFrame(follow);
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
        cancelAnimationFrame(tick);
        view?.dispose();
        view = null;
      },
    };
  },
};

function iconButton(icon: IconName, title: string): HTMLButtonElement {
  const b = h("button", { className: "tl-icon", title });
  b.innerHTML = ICONS[icon];
  b.setAttribute("aria-label", title.replace(/ \(.*\)$/, ""));
  return b;
}

/** A track toggle: shows its on icon while pressed. */
function toggleButton(on: boolean, onIcon: IconName, offIcon: IconName, title: string, act: () => void): HTMLButtonElement {
  const b = iconButton(on ? onIcon : offIcon, title);
  b.classList.add("tl-track-toggle");
  b.setAttribute("aria-pressed", String(on));
  // Keep focus on the timeline, so its keys keep working after a click.
  b.onmousedown = (ev) => ev.preventDefault();
  b.onclick = act;
  return b;
}
